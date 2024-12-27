use color_eyre::eyre::Result;
use domain::{
    base::{iana::Class, Name, Question, Rtype},
    rdata::Mx,
    resolv::StubResolver,
};
use smtp_proto::{MailFrom, RcptTo, Request};
use tokio::net::TcpStream;
use tracing::{debug, info, warn};

use crate::stream::Stream;

pub async fn send_group(host: &str, msg: String, members: &Vec<String>, from: &str) {
    for recipient in members {
        let server = match recipient.split('@').nth(1) {
            Some(v) => v.trim_end_matches('>'),
            None => continue,
        };

        match send(
            host,
            &msg,
            &recipient,
            &from,
            server.to_string(),
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
    msg: &str,
    to: &str,
    from: &str,
    server_override: String,
    server_port: Option<u16>,
    server: String,
) -> Result<()> {
    let stream = &mut establish_smtp_connection(server_override, server_port, host, server).await?;

    let mut mail_from = MailFrom::default();
    mail_from.address = from;

    let mut rcpt_to = RcptTo::default();
    rcpt_to.address = to;

    let _response = stream.recieve_response().await?;
    stream
        .send_request(Request::Mail { from: mail_from })
        .await?;
    let _response = stream.recieve_response().await?;
    stream.send_request(Request::Rcpt { to: rcpt_to }).await?;
    let _response = stream.recieve_response().await?;
    stream.send_request::<String>(Request::Data).await?;
    let _response = stream.recieve_response().await?;
    stream.send_mail(msg.as_bytes()).await?;
    let _response = stream.recieve_response().await?;
    stream.send_request::<String>(Request::Quit).await?;

    info!("Sent mail to {to}");
    Ok(())
}

async fn establish_smtp_connection(
    server: String,
    server_port: Option<u16>,
    host: &str,
    tls: String,
) -> Result<Stream> {
    let server_port = server_port.unwrap_or(25);

    let address = get_address(&server).await?;
    let stream = TcpStream::connect(format!("{address}:{server_port}")).await?;
    let mut stream = Stream::Tcp(stream);

    let _response = stream.recieve_response().await?;
    stream.send_request(Request::Ehlo { host }).await?;

    let capabilities = stream.recieve_capabilities().await?;

    let supports_tls = capabilities.contains(&"STARTTLS".to_string());

    if supports_tls {
        stream.send_request::<String>(Request::StartTls).await?;
        let _response = stream.recieve_response().await?;

        let mut stream = stream.start_tls_client(tls).await?;

        stream.send_request(Request::Ehlo { host }).await?;
        return Ok(stream);
    } else {
        return Ok(stream);
    }
}

async fn get_address(server: &str) -> Result<String> {
    let ip: String;
    if server.starts_with('[') && server.ends_with(']') {
        let server = server
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_string();
        ip = server;
    } else {
        ip = lookup_mx(server).await?;
    }

    Ok(ip)
}

async fn lookup_mx(server: &str) -> Result<String> {
    let resolver = StubResolver::new();
    let name = Name::bytes_from_str(server)?;
    let question = Question::new(name, Rtype::MX, Class::IN);
    let answer = resolver.query(question).await?;
    let answer = answer.answer()?.next().unwrap()?;
    let answer = answer.to_record::<Mx<_>>()?.unwrap();
    let answer = answer.data().exchange().to_string();

    Ok(answer)
}
