use std::net::SocketAddr;

use smtp_proto::{Request, Response};
use std::io::Result;
use tokio::{io::AsyncWriteExt, net::TcpStream};
use tracing::info;
use trust_dns_resolver::{
    name_server::{GenericConnector, TokioRuntimeProvider},
    AsyncResolver,
};

static CAPABILITIES: &'static [u8] = br#"250-Helu!
250-SIZE 14680064
250-STARTTLS
250 ENHANCEDSTATUSCODES
"#;

use crate::{config::ServerConfig, mail::Mail, stream::Stream};

pub async fn handle_client(
    _addr: SocketAddr,
    stream: TcpStream,
    config: &ServerConfig,
    resolver: &AsyncResolver<GenericConnector<TokioRuntimeProvider>>,
) -> Result<()> {
    let mut stream = Stream::Tcp(stream);

    let host = init_connection(&mut stream).await?;
    info!("Got connection from {host}.");

    let mut request = stream.recieve_request().await?;

    let mut starttls = false;
    match &request {
        &Request::StartTls => starttls = true,
        _ => {}
    };

    if starttls {
        stream.send_response(Response::new(220, 2, 2, 0, "Go ahead")).await?;
        stream = stream.start_tls().await?;

        init_connection(&mut stream).await?;
        request = stream.recieve_request().await?;
    }

    loop {
        let mail = recieve_mail(&mut stream, request.clone()).await?;
        match mail.handle(config, resolver).await {
            Ok(_) => {
                stream
                    .send_response(Response::new(
                        250,
                        2,
                        6,
                        0,
                        "Message Recieved Succesfully!!",
                    ))
                    .await?
            }
            Err(_) => {
                stream
                    .send_response(Response::new(552, 5, 5, 0, "Woopsie"))
                    .await?
            }
        };
    }
}

async fn recieve_mail(stream: &mut Stream, to: Request<String>) -> Result<Mail> {
    let sender = get_sender(stream, to).await?;
    let recipients = get_recipients(stream).await?;

    let data = String::from_utf8_lossy(&stream.recieve_mail().await?).to_string();

    let mail = Mail {
        sender,
        recipients,
        data,
    };

    Ok(mail)
}

async fn get_recipients(stream: &mut Stream) -> Result<Vec<String>> {
    let mut recipients: Vec<String> = Vec::new();
    loop {
        info!("Getting reciever");

        let request = stream.recieve_request().await?;
        let to = match request {
            Request::Rcpt { to } => to,
            Request::Data => break,
            _ => {
                stream.protocol_error().await?;
                continue;
            }
        };

        let address = to.address;

        stream
            .send_response(Response::new(
                250,
                2,
                1,
                0,
                format!("Reciever {address} okay"),
            ))
            .await?;

        recipients.push(address)
    }
    stream
        .send_response(Response::new(354, 2, 0, 0, "End with CRLF.CRLF"))
        .await?;
    return Ok(recipients);
}

async fn get_sender(stream: &mut Stream, mut request: Request<String>) -> Result<String> {
    let mut is_first = true;
    loop {
        info!("Getting Sender");
        if !is_first {
            request = stream.recieve_request().await?;
        } else {
            is_first = false;
        }
        let from = match request {
            Request::Mail { from } => from,
            _ => {
                stream.protocol_error().await?;
                continue;
            }
        };

        let address = from.address;

        stream
            .send_response(Response::new(
                250,
                2,
                1,
                0,
                format!("Originator {address} okay"),
            ))
            .await?;

        return Ok(address);
    }
}

async fn init_connection(stream: &mut Stream) -> Result<String> {
    stream
        .send_response(Response::new(220, 2, 2, 0, "SMTP mailing-list"))
        .await?;

    info!("Greeted");

    loop {
        let request = stream.recieve_request().await?;

        let (host, esmtp) = match request {
            Request::Ehlo { host } => (host, true),
            Request::Helo { host } => (host, false),
            _request => {
                stream.protocol_error().await?;
                continue;
            }
        };

        info!("Host: {host}");
        info!("ESMTP: {esmtp}");

        if esmtp {
            (*stream.deref()).write_all(CAPABILITIES).await?;
        } else {
            stream
                .send_response(Response::new(250, 2, 5, 0, format!("Welcome {host}")))
                .await?;
        }

        info!("Finished introduction");

        return Ok(host);
    }
}
