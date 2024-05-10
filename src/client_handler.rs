use color_eyre::eyre::Result;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tracing::info;
use trust_dns_resolver::TokioAsyncResolver;

use crate::{
    client_handler::states::{
        connected::handle_connected, from::handle_from, idle::handle_idle,
        recieving::handle_recieving,
    },
    config::ServerConfig,
    io::tx,
};

use states::{
    connected::ConnectedState, from::FromState, idle::IdleState, recieving::RecievingState,
};

mod states;

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
pub enum State {
    Connected(ConnectedState),
    Idle(IdleState),
    From(FromState),
    Recieving(RecievingState),
}

pub async fn handle_client(
    addr: SocketAddr,
    mut stream: TcpStream,
    config: &ServerConfig,
    resolver: &TokioAsyncResolver,
) -> Result<()> {
    info!("Handling connection from: {addr}");
    let stream = &mut stream;

    let init_msg = format!("220 {} SMTP Postfix", config.hostname);
    tx(stream, init_msg, false, true).await?;
    let mut current_state = State::Connected(ConnectedState);
    let mut messages = Vec::new();

    use State::{Connected as C, From as F, Idle as I, Recieving as R};

    loop {
        current_state = match current_state {
            C(state) => handle_connected(stream, state).await?,
            I(state) => handle_idle(stream, state).await?,
            F(state) => handle_from(stream, state).await?,
            R(state) => {
                handle_recieving(
                    stream,
                    state,
                    &mut messages,
                    &config.hostname,
                    resolver,
                    config,
                )
                .await?
            }
        }
    }
}

fn parse_message(msg: String) -> Option<Message> {
    let msg = msg.trim_end();
    let mut parts = msg.splitn(2, ' ');
    let command = parts.next()?.to_uppercase();
    let args = parts
        .next()
        .unwrap_or(" ")
        .split(' ')
        .map(|x| x.to_owned())
        .collect();
    Some(Message { command, args })
}
