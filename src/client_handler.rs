use color_eyre::eyre::Result;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tracing::{error, info};
use trust_dns_resolver::TokioAsyncResolver;

use crate::{
    client_handler::states::{
        connected::handle_connected, from::handle_from, idle::handle_idle,
        recieving::handle_recieving,
    },
    config::ServerConfig,
    io::tx,
    plugins, AsyncStream,
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

#[derive(Debug)]
pub struct State {
    pub state_type: StateType,
    pub stream: Box<dyn AsyncStream>,
}

#[derive(Debug, Clone)]
pub enum StateType {
    Connected(ConnectedState),
    Idle(IdleState),
    From(FromState),
    Recieving(RecievingState),
}

pub async fn handle_client(
    addr: SocketAddr,
    stream: TcpStream,
    config: &ServerConfig,
    resolver: &TokioAsyncResolver,
) -> Result<()> {
    info!("Handling connection from: {addr}");
    let mut stream = stream;

    let mut loaded_plugins = Vec::new();

    for plugin in config.plugins.clone() {
        let loaded_plugin = match plugins::get_plugin(&plugin) {
            Ok(v) => v,
            Err(_e) => {
                error!("Unable to load: {plugin}");
                error!("{_e}");
                continue;
            }
        };
        loaded_plugins.push(loaded_plugin);
    }

    let init_msg = format!("220 {} SMTP Postfix", config.hostname);
    tx(&mut stream, init_msg, false, true).await?;
    let mut current_state = State {
        state_type: StateType::Connected(ConnectedState),
        stream: Box::new(stream),
    };
    let mut messages = Vec::new();

    use StateType::{Connected as C, From as F, Idle as I, Recieving as R};

    loop {
        current_state = match &current_state.state_type {
            C(_) => handle_connected(current_state).await?,
            I(_) => handle_idle(current_state).await?,
            F(_) => handle_from(current_state).await?,
            R(_) => {
                handle_recieving(
                    current_state,
                    &mut messages,
                    &config.hostname,
                    resolver,
                    config,
                    &loaded_plugins,
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
