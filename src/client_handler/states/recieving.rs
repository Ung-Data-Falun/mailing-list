use color_eyre::eyre::Result;
use dlopen::wrapper::Container;
use tracing::{debug, error, info, warn};
use trust_dns_resolver::TokioAsyncResolver;

use crate::{
    client_handler::{states::idle::IdleState, Mail, State, StateType},
    config::ServerConfig,
    error::Error,
    io::{rx, tx},
    plugins::{PluginApi, UnwrappedPlugin},
    send_mail::{self, send_group},
};

#[derive(Debug, Clone)]
pub struct RecievingState {
    pub foreign_host: String,
    pub from: String,
    pub to: Vec<String>,
}

impl From<&State> for RecievingState {
    fn from(value: &State) -> Self {
        match &value.state_type {
            StateType::Recieving(v) => v.clone(),
            _ => panic!(),
        }
    }
}

pub async fn handle_recieving(
    mut state: State,
    messages: &mut Vec<Mail>,
    host: &str,
    resolver: &TokioAsyncResolver,
    config: &ServerConfig,
    loaded_plugins: &Vec<(UnwrappedPlugin, Container<PluginApi>)>,
) -> Result<State> {
    let recieving_state: RecievingState = (&state).into();
    let stream = &mut state.stream;
    let mut message = String::new();
    let mut current_line = String::new();
    loop {
        message += &current_line;
        if current_line.ends_with(".\r\n") {
            break;
        }
        current_line = rx(stream, true).await?;
    }
    messages.push(Mail {
        from: recieving_state.from.clone(),
        to: recieving_state.to.clone(),
        payload: message.clone(),
    });

    for plugin in loaded_plugins {
        if let Some(message_handler) = plugin.0.message_handler {
            message_handler(message.clone());
        }
    }

    info!("Recieved mail from {}", recieving_state.from);

    let mut is_forwarding = true;
    let lists = &config.lists;
    for mail in lists.keys() {
        if recieving_state.to.contains(mail) {
            info!("Sending to everyone subscribing to {mail}");
            send_group(resolver, host, message.clone(), &lists[mail].members, mail).await;
            is_forwarding = false;
        }
    }
    if is_forwarding && config.forwarding.is_some() && config.forwarding.clone().unwrap().enable {
        let server = config.forwarding.clone().unwrap();
        info!("Forwarding to {}", &recieving_state.to[0]);
        let port = match server.enable {
            true => server.port,
            false => None,
        };
        let server = match server.enable {
            false => match recieving_state.to[0].clone().split('@').nth(1) {
                Some(v) => v.trim_end_matches('>'),
                None => return Err(Error::InvalidMail.into()),
            }
            .to_string(),
            true => match server.server.clone() {
                Some(v) => v,
                None => {
                    error!("Config enables mail forwarding but doesn't provide an address to forward to.");
                    panic!();
                }
            },
        };
        match send_mail::send(
            host,
            resolver,
            &message,
            &recieving_state.to[0],
            &recieving_state.from,
            &server,
            port,
            config.forwarding.clone().unwrap().server_tls,
        )
        .await
        {
            Ok(_) => {}
            Err(_e) => {
                warn!("Couldn't forward mail to mail-server");
                debug!("{_e}");
            }
        };
    }

    tx(
        stream,
        format!("250 Thank you for the message! I will make sure that it comes through"),
        false,
        true,
    )
    .await?;

    Ok(State {
        state_type: StateType::Idle(IdleState {
            foreign_host: recieving_state.foreign_host,
        }),
        stream: state.stream,
    })
}
