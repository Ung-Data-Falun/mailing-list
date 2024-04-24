use clap::Parser;
use cli::Cli;
use client_handler::handle_client;
use color_eyre::eyre::Result;
use config::get_config;
use members::get_members;
use tokio::{io::BufStream, net::TcpListener, runtime::Runtime};
use tracing::{debug, info, warn, Level};
use trust_dns_resolver::{
    config::{ResolverConfig, ResolverOpts},
    TokioAsyncResolver,
};

mod cli;
mod client_handler;
mod config;
mod error;
mod io;
mod members;
mod send_mail;

fn init() -> Result<()> {
    let format = tracing_subscriber::fmt::format()
        .with_line_number(true)
        .with_source_location(true);
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .event_format(format)
        .init();
    color_eyre::install()?;
    Ok(())
}

fn main() -> Result<()> {
    let runtime = Runtime::new()?;
    let resolver = runtime.block_on(async {
        TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default())
    });
    runtime.block_on(run(resolver))?;

    Ok(())
}

async fn run(resolver: TokioAsyncResolver) -> Result<()> {
    init()?;

    let args = Cli::parse();
    let config = get_config(args.config.as_deref())?;
    let members = get_members(config.member_file.as_deref())?;

    let port = config.port.unwrap_or(25);
    let ip = config.ip.clone().unwrap_or("0.0.0.0".to_string());

    info!("Starting mailing-list on port {port}");
    let listener = TcpListener::bind(format!("{ip}:{port}")).await?;
    info!("Started mailing-list on port {port}");

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
        let config = config.clone();
        let members = members.clone();
        let resolver = resolver.clone();
        tokio::spawn(async move {
            match handle_client(addr, BufStream::new(stream), &config, &members, &resolver).await {
                Ok(_) => {}
                Err(e) => warn!("{e}"),
            };
        });
    }
}
