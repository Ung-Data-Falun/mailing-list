use color_eyre::eyre::Result;
use tracing::debug;

use crate::{
    client_handler::{parse_message, states::from::FromState, State, StateType},
    error::Error,
    io::{rx, tx},
};

impl From<&State> for IdleState {
    fn from(value: &State) -> Self {
        match &value.state_type {
            StateType::Idle(idle_state) => idle_state.clone(),
            _ => panic!(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IdleState {
    pub foreign_host: String,
}

pub async fn handle_idle(mut state: State) -> Result<State> {
    let idle_state: IdleState = (&state).into();
    let stream = &mut state.stream;
    let message = rx(stream, false).await?;
    let message = match parse_message(message.clone()) {
        Some(v) => v,
        None => return Err(Error::InvalidCommand(Some(message)).into()),
    };
    let command = message.command;
    match &command as &str {
        "MAIL" => {}
        "RSET" => {
            tx(stream, "250 Okay".to_string(), false, true).await?;
        }
        "QUIT" => {
            tx(
                stream,
                format!(
                    "221 Bye bye {}. Nice to talk to you :3",
                    idle_state.foreign_host
                ),
                false,
                true,
            )
            .await?;
            return Err(Error::Quit.into());
        }
        "HELO" => {
            tx(
                stream,
                format!("250 Hello {}", idle_state.foreign_host),
                false,
                true,
            )
            .await?;
            return Ok(State {
                state_type: StateType::Idle(idle_state),
                stream: state.stream,
            });
        }
        "EHLO" => {
            tx(
                stream,
                format!("250 Hello {}", idle_state.foreign_host),
                false,
                true,
            )
            .await?;
            return Ok(State {
                state_type: StateType::Idle(idle_state),
                stream: state.stream,
            });
        }
        "STARTTLS" => {}
        _ => return Err(Error::InvalidCommand(Some(command)).into()),
    }

    let from = message.args.join(" ");
    let from = match from.strip_prefix("FROM:") {
        Some(v) => v.to_string(),
        None => return Err(Error::InvalidCommand(Some(from)).into()),
    };
    let from = from.trim();
    let from = from.trim_start_matches('<');
    let from = from.trim_end_matches('>');
    let from = from.to_string();
    debug!("Recieving a mail from {from}");
    tx(
        stream,
        format!("250 who should get your message {from}?"),
        false,
        true,
    )
    .await?;

    Ok(State {
        state_type: StateType::From(FromState {
            foreign_host: idle_state.foreign_host,
            from,
            to: Vec::new(),
        }),
        stream: state.stream,
    })
}
