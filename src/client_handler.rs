use color_eyre::eyre::Result;
use std::net::SocketAddr;
use tokio::{io::BufStream, net::TcpStream};
use tracing::{debug, info};
use trust_dns_resolver::TokioAsyncResolver;

use crate::{
    config::ServerConfig,
    error::Error,
    io::{rx, tx},
    members::Members,
    send_mail::send_group,
};

type FQDN = String;
type Sender = String;
type Reciever = String;

#[derive(Debug, Clone)]
struct Message {
    command: String,
    args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Mail {
    pub from: Sender,
    pub to: Vec<Reciever>,
    pub payload: String,
}

#[derive(Debug, Clone)]
enum State {
    Connected,
    Idle(FQDN),
    From(FQDN, Sender, Vec<Reciever>),
    Recieving(FQDN, Sender, Vec<Reciever>),
}

pub async fn handle_client(
    addr: SocketAddr,
    mut stream: BufStream<TcpStream>,
    config: &ServerConfig,
    members: &Members,
    resolver: &TokioAsyncResolver,
) -> Result<()> {
    info!("Handling connection from: {addr}");
    let stream = &mut stream;

    let init_msg = format!("220 {} SMTP Postfix", config.hostname);
    tx(stream, init_msg).await?;
    let mut current_state = State::Connected;
    let mut messages = Vec::new();

    use State::{Connected as C, From as F, Idle as I, Recieving as R};

    loop {
        debug!("Current state: {current_state:?}");
        current_state = match current_state {
            C => handle_connected(stream).await?,
            I(fqdn) => handle_idle(stream, fqdn).await?,
            F(fqdn, sender, recievers) => handle_from(stream, fqdn, sender, recievers).await?,
            R(fqdn, sender, recievers) => {
                handle_recieving(
                    stream,
                    fqdn,
                    sender,
                    recievers,
                    &mut messages,
                    members,
                    &config.hostname,
                    resolver,
                )
                .await?
            }
        }
    }
}

async fn handle_connected(stream: &mut BufStream<TcpStream>) -> Result<State> {
    let message = rx(stream).await?;
    let message = match parse_message(message) {
        Some(v) => v,
        None => return Err(Error::InvalidCommand.into()),
    };
    let command = message.command;
    match &command as &str {
        "HELO" => {}
        "EHLO" => {}
        _ => return Err(Error::InvalidCommand.into()),
    };

    let foreign_host = match message.args.get(0) {
        Some(v) => v.to_string(),
        None => return Err(Error::InvalidCommand.into()),
    };

    debug!("Foreign host: {foreign_host}");

    tx(
        stream,
        format!(
            "250 Hello {}, nice to meet you. I'm running mailing-list. Any questions? :)",
            foreign_host
        ),
    )
    .await?;

    Ok(State::Idle(foreign_host))
}

async fn handle_idle(stream: &mut BufStream<TcpStream>, fqdn: FQDN) -> Result<State> {
    let message = rx(stream).await?;
    let message = match parse_message(message) {
        Some(v) => v,
        None => return Err(Error::InvalidCommand.into()),
    };
    let command = message.command;
    match &command as &str {
        "MAIL" => {}
        "RSET" => {
            tx(stream, "250 Okay".to_string()).await?;
        }
        "QUIT" => {
            tx(
                stream,
                format!("221 Bye bye {fqdn}. Nice to talk to you :3"),
            )
            .await?;
            return Err(Error::Quit.into());
        }
        _ => return Err(Error::InvalidCommand.into()),
    }

    let from = message.args.join(" ");
    let from = match from.strip_prefix("FROM:") {
        Some(v) => v.to_string(),
        None => return Err(Error::InvalidCommand.into()),
    };
    let from = from.trim();
    let from = from.trim_start_matches('<');
    let from = from.trim_end_matches('>');
    let from = from.to_string();
    debug!("Recieving a mail from {from}");
    tx(stream, format!("250 who should get your message {from}?")).await?;

    Ok(State::From(fqdn, from, Vec::new()))
}

async fn handle_from(
    stream: &mut BufStream<TcpStream>,
    fqdn: FQDN,
    sender: Sender,
    mut recievers: Vec<Reciever>,
) -> Result<State> {
    let message = rx(stream).await?;
    let message = match parse_message(message) {
        Some(v) => v,
        None => return Err(Error::InvalidCommand.into()),
    };
    let command = message.command;
    match &command as &str {
        "RCPT" => {}
        "DATA" => {
            tx(
                stream,
                "354 Type your message. End with <CR><LF>.<CR><LF>".to_string(),
            )
            .await?;
            debug!("Recieving message");
            return Ok(State::Recieving(fqdn, sender, recievers));
        }
        _ => return Err(Error::InvalidCommand.into()),
    }

    let to = message.args.join(" ");
    let to = match to.strip_prefix("TO:") {
        Some(v) => v.to_string(),
        None => return Err(Error::InvalidCommand.into()),
    };
    let to = to.trim();
    let to = to.trim_start_matches('<');
    let to = to.trim_end_matches('>');
    let to = to.to_string();
    debug!("Sending the mail to {to}");
    tx(
        stream,
        format!("250 I will make sure your message gets to {to} :3"),
    )
    .await?;

    recievers.push(to);
    Ok(State::From(fqdn, sender, recievers))
}

async fn handle_recieving(
    stream: &mut BufStream<TcpStream>,
    fqdn: FQDN,
    sender: Sender,
    recievers: Vec<Reciever>,
    messages: &mut Vec<Mail>,
    members: &Members,
    host: &str,
    resolver: &TokioAsyncResolver,
) -> Result<State> {
    let mut message = String::new();
    let mut current_line = String::new();
    loop {
        message += &current_line;
        message += "\n";
        current_line = rx(stream).await?;
        if current_line == "." {
            break;
        }
    }
    debug!("{message}");
    messages.push(Mail {
        from: sender,
        to: recievers.clone(),
        payload: message.clone(),
    });
    send_group(resolver, host, message, members, recievers[0].clone()).await;
    tx(
        stream,
        format!("250 Thank you for the message! I will make sure that it comes through"),
    )
    .await?;
    Ok(State::Idle(fqdn))
}

fn parse_message(msg: String) -> Option<Message> {
    let mut parts = msg.splitn(2, ' ');
    let command = parts.next()?.to_string();
    let args = parts
        .next()
        .unwrap_or(" ")
        .split(' ')
        .map(|x| x.to_owned())
        .collect();
    Some(Message { command, args })
}
