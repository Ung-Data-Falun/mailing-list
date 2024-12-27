use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use color_eyre::eyre::Result;
use rustls::pki_types::ServerName;
use tokio::net::TcpStream;
use tracing::{debug, info, warn};
use trust_dns_resolver::TokioAsyncResolver;

use crate::{
    error::Error,
    io::{rx, tx},
    AsyncStream,
};

pub async fn send_group(
    resolver: &TokioAsyncResolver,
    host: &str,
    msg: String,
    members: &Vec<String>,
    from: &str,
) {
    for recipient in members {
        let server = match recipient.split('@').nth(1) {
            Some(v) => v.trim_end_matches('>'),
            None => continue,
        };

        match send(
            host,
            resolver,
            &msg,
            &recipient,
            &from,
            server,
            None,
            server.to_string(),
        )
        .await
        {
            Ok(_) => {}
            Err(_e) => {
                warn!("Couldn't send mail to {recipient}");
                debug!("Error: {_e}");
            }
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
    local_tls_address: String,
) -> Result<()> {
    let stream = &mut establish_smtp_connection(
        server.to_string(),
        server_port,
        resolver,
        host,
        local_tls_address,
    )
    .await?;

    tx(stream, format!("EHLO {host}"), false, true).await?;
    rx(stream, false).await?;
    tx(stream, format!("MAIL FROM:{from}"), false, true).await?;
    rx(stream, false).await?;
    tx(stream, format!("RCPT TO:{to}"), false, true).await?;
    rx(stream, false).await?;
    tx(stream, format!("DATA"), false, true).await?;
    rx(stream, false).await?;
    tx(stream, msg.to_string(), true, false).await?;
    rx(stream, false).await?;
    tx(stream, format!("QUIT"), false, true).await?;

    info!("Sent mail to {to}");
    Ok(())
}

async fn establish_smtp_connection(
    server: String,
    server_port: Option<u16>,
    resolver: &TokioAsyncResolver,
    host: &str,
    local_tls_address: String,
) -> Result<Box<dyn AsyncStream>> {
    let server_port = server_port.unwrap_or(25);
    let ip: IpAddr;
    let dns_name: ServerName<'_>;
    if server.starts_with('[') && server.ends_with(']') {
        let server = server
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_string();
        dns_name = local_tls_address.try_into()?;
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

    rx(&mut stream, false).await?;
    tx(&mut stream, format!("EHLO {host}"), false, true).await?;
    let supports_tls = rx(&mut stream, false)
        .await?
        .split("\r\n")
        .collect::<Vec<&str>>()
        .contains(&"250-STARTTLS");
    if supports_tls {
        tx(&mut stream, "STARTTLS".to_string(), false, true).await?;
        rx(&mut stream, false).await?;
        let root_store =
            rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        let rc_config = Arc::new(config);

        let connector = tokio_rustls::TlsConnector::from(rc_config);
        return Ok(Box::new(connector.connect(dns_name.clone(), stream).await?));
    } else {
        return Ok(Box::new(stream));
    }
}
