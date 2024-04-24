use clap::Parser;
use cli::Cli;
use client_handler::handle_client;
use color_eyre::eyre::Result;
use config::get_config;
use tokio::{io::BufStream, net::TcpListener};
use tracing::{debug, info, warn, Level};

mod cli;
mod client_handler;
mod config;

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

#[tokio::main]
async fn main() -> Result<()> {
    init()?;

    let args = Cli::parse();
    let config = get_config(args.config.as_deref())?;

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
        tokio::spawn(async move {
            match handle_client(addr, BufStream::new(stream), &config).await {
                Ok(_) => {}
                Err(e) => warn!("{e}"),
            };
        });
    }
}
