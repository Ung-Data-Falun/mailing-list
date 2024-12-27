#![doc = include_str!("../README.md")]

#[macro_use]
extern crate dlopen_derive;

use std::{fmt::Debug, sync::Mutex, time::Duration};

use clap::Parser;
use cli::Cli;
use client_handler::handle_client;
use color_eyre::eyre::Result;
use config::get_config;
use dlopen::wrapper::Container;
use plugins::PluginApi;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    runtime::Runtime, time::timeout,
};
use tokio_rustls::{client, server, TlsStream};
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};
use trust_dns_resolver::{
    config::{ResolverConfig, ResolverOpts},
    TokioAsyncResolver,
};

mod cli;
mod client_handler;
mod config;
mod error;
mod io;
mod mail;
mod plugins;
mod send_mail;
mod stream;

trait AsyncStream: AsyncRead + AsyncWrite + std::marker::Unpin + Send + Debug {}
impl AsyncStream for TcpStream {}
impl AsyncStream for TlsStream<TcpStream> {}
impl AsyncStream for client::TlsStream<TcpStream> {}
impl AsyncStream for server::TlsStream<TcpStream> {}

pub static PLUGINS: Mutex<Option<Vec<(mlpa::Plugin, Container<PluginApi>)>>> = Mutex::new(None);

fn main() -> Result<()> {
    let runtime = Runtime::new()?;
    let resolver = runtime.block_on(async {
        TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default())
    });
    runtime.block_on(run(resolver))?;

    Ok(())
}

async fn run(resolver: TokioAsyncResolver) -> Result<()> {
    let format_stdout = tracing_subscriber::fmt::format()
        .with_line_number(true)
        .with_source_location(false);

    let (log, _guard) = tracing_appender::non_blocking(std::fs::File::create("log.txt")?);
    let (stdout, _guard) = tracing_appender::non_blocking(std::io::stdout());
    let format_log = tracing_subscriber::fmt::format()
        .with_line_number(true)
        .with_source_location(false);

    let filter_log = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(Level::DEBUG.into())
        .from_env_lossy();
    let log_layer = tracing_subscriber::fmt::layer()
        .with_writer(log)
        .event_format(format_log)
        .with_line_number(true)
        .with_filter(filter_log);
    let filter_stdout = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(Level::INFO.into())
        .from_env_lossy();
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(stdout)
        .event_format(format_stdout)
        .with_filter(filter_stdout);

    tracing_subscriber::Registry::default()
        .with(log_layer)
        .with(stdout_layer)
        .init();
    info!("Started Logger");

    color_eyre::install()?;

    let args = Cli::parse();

    let config = get_config(args.config.as_deref())?;
    let port = config.port.unwrap_or(25);
    let ip = config.ip.clone().unwrap_or("0.0.0.0".to_string());

    info!("Starting mailing-list on port {port}");
    let listener = TcpListener::bind(format!("{ip}:{port}")).await?;
    info!("Started mailing-list on port {port}");

    let mut plugins = Vec::new();

    for plugin in config.plugins.clone() {
        let plugin = match plugins::get_plugin(&plugin) {
            Ok(v) => v,
            Err(_e) => {
                error!("Unable to load: {plugin}");
                error!("{_e}");
                continue;
            }
        };
        if let mlpa::Optional::Some(on_start) = plugin.0.on_start {
            unsafe {
                on_start();
            }
        };
        plugins.push(plugin);
    }

    *PLUGINS.lock().unwrap() = Some(plugins);

    loop {
        let (stream, addr) = match listener.accept().await {
            Ok(v) => {
                debug!("Connection from: {}", v.1);
                v
            }
            Err(e) => {
                warn!("{e}");
                continue;
            }
        };

        let config = get_config(args.config.as_deref())?;
        tokio::spawn(timeout(Duration::from_secs(10), async move {
            match handle_client(addr, stream, &config).await {
                Ok(_) => {}
                Err(e) => warn!("Error: {e}"),
            };
        }));
    }
}
