use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use color_eyre::eyre::Result;
use rustls::pki_types::ServerName;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};
use tokio_rustls::client::TlsStream;
use tracing::{debug, info, warn};
use trust_dns_resolver::TokioAsyncResolver;

use crate::{
    error::Error,
    io::{rx, tx},
};

trait AsyncStream: AsyncRead + AsyncWrite + std::marker::Unpin + Send {}
impl AsyncStream for TcpStream {}
impl AsyncStream for TlsStream<TcpStream> {}

pub async fn send_group(
    resolver: &TokioAsyncResolver,
    host: &str,
    msg: String,
    members: &Vec<String>,
    from: String,
) {
    for recipient in members {
        let server = match recipient.split('@').nth(1) {
            Some(v) => v.trim_end_matches('>'),
            None => continue,
        };

        match send(host, resolver, &msg, &recipient, &from, server, None, None).await {
            Ok(_) => debug!("Sent mail to {recipient}"),
            Err(_e) => {
                warn!("Couldn't send mail to {recipient}");
                debug!("Error: {_e}");
            },
        };
    }
}

pub async fn send(
    host: &str,
    resolver: &TokioAsyncResolver,
    msg: &str,
    to: &str,
    from: &str,
    server: &str,
    server_port: Option<u16>,
    local_tls_address: Option<String>
) -> Result<()> {
    let stream = &mut establish_smtp_connection(server.to_string(), server_port, resolver, host, local_tls_address).await?;

    tx(stream, format!("EHLO {host}")).await?;
    rx(stream).await?;
    tx(stream, format!("MAIL FROM:{from}")).await?;
    rx(stream).await?;
    tx(stream, format!("RCPT TO:{to}")).await?;
    rx(stream).await?;
    tx(stream, format!("DATA")).await?;
    rx(stream).await?;
    for line in msg.trim_end().split("\r\n") {
        tx(stream, line.to_string()).await?;
    }
    rx(stream).await?;
    tx(stream, format!("QUIT")).await?;
    rx(stream).await?;

    Ok(())
}

async fn establish_smtp_connection(
    server: String,
    server_port: Option<u16>,
    resolver: &TokioAsyncResolver,
    host: &str,
    local_tls_address: Option<String>
) -> Result<Box<dyn AsyncStream>> {
    let server_port = server_port.unwrap_or(25);
    let ip: IpAddr;
    let dns_name: ServerName<'_>;
    if server.starts_with('[') && server.ends_with(']') {
        let server = server
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_string();
        dns_name = local_tls_address.unwrap_or(server.clone()).try_into()?;
        ip = server.parse::<IpAddr>()?;
    } else {
        let lookup = resolver.mx_lookup(server.clone()).await?;
        let record = match lookup.iter().next() {
            Some(v) => v,
            None => return Err(Error::InvalidMail.into()),
        };
        let record = record.exchange().to_utf8();
        dns_name = record.clone().try_into()?;
        let record = match resolver.lookup_ip(record).await?.iter().next() {
            Some(v) => v,
            None => return Err(Error::InvalidMail.into()),
        };
        ip = record.into();
    }

    let address = SocketAddr::new(ip, server_port);
    let mut stream = TcpStream::connect(address).await?;

    rx(&mut stream).await?;
    tx(&mut stream, format!("EHLO {host}")).await?;
    let supports_tls = rx(&mut stream)
        .await?
        .split("\r\n")
        .collect::<Vec<_>>()
        .contains(&"250-STARTTLS");
    if supports_tls {
        tx(&mut stream, "STARTTLS".to_string()).await?;
        rx(&mut stream).await?;
        let root_store =
            rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        let rc_config = Arc::new(config);

        info!("Starting TLS connection with {ip}");
        let connector = tokio_rustls::TlsConnector::from(rc_config);
        info!("Started TLS connection with {ip}");
        return Ok(Box::new(connector.connect(dns_name.clone(), stream).await?));
    } else {
        return Ok(Box::new(stream));
    }
}
