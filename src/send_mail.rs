use std::net::{IpAddr, SocketAddr};

use color_eyre::eyre::Result;
use tokio::{io::BufStream, net::TcpStream};
use tracing::{debug, warn};
use trust_dns_resolver::TokioAsyncResolver;

use crate::{
    error::Error,
    io::{rx, tx},
    members::Members,
};

pub async fn send_group(
    resolver: &TokioAsyncResolver,
    host: &str,
    msg: String,
    members: &Members,
    from: String,
) {
    for recipient in &members.members {
        match send(host, resolver, &msg, &recipient, &from).await {
            Ok(_) => debug!("Sent mail to {recipient}"),
            Err(_) => warn!("Couldn't send mail to {recipient}"),
        };
    }
}

async fn send(
    host: &str,
    resolver: &TokioAsyncResolver,
    msg: &str,
    to: &str,
    from: &str,
) -> Result<()> {
    let server = match to.split('@').nth(1) {
        Some(v) => v.trim_end_matches('>'),
        None => {
            return Err(Error::InvalidMail.into());
        }
    };

    let ip: IpAddr;
    if server.starts_with('[') && server.ends_with(']') {
        ip = server.parse()?;
    } else {
        let lookup = resolver.mx_lookup(server).await?;
        let record = match lookup.iter().next() {
            Some(v) => v,
            None => return Err(Error::InvalidMail.into()),
        };
        let record = record.exchange().to_utf8();
        let record = match resolver.lookup_ip(record).await?.iter().next() {
            Some(v) => v,
            None => return Err(Error::InvalidMail.into()),
        };
        ip = record.into();
    }

    let address = SocketAddr::new(ip, 25);
    debug!("test");
    debug!("{address}");
    let tcp_stream = TcpStream::connect(address).await?;
    let stream = &mut BufStream::new(tcp_stream);

    rx(stream).await?;
    tx(stream, format!("HELO {host}")).await?;
    rx(stream).await?;
    tx(stream, format!("MAIL FROM:{from}")).await?;
    rx(stream).await?;
    tx(stream, format!("RCPT TO:{to}")).await?;
    rx(stream).await?;
    tx(stream, format!("DATA")).await?;
    for line in msg.split('\n') {
        tx(stream, line.to_string()).await?;
    }
    tx(stream, format!("")).await?;
    tx(stream, format!(".")).await?;
    rx(stream).await?;
    tx(stream, format!("QUIT")).await?;
    rx(stream).await?;

    Ok(())
}
