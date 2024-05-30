use color_eyre::eyre::Result;
use tracing::debug;

use crate::{
    client_handler::{parse_message, states::recieving::RecievingState, State, StateType},
    error::Error,
    io::{rx, tx},
};

#[derive(Debug, Clone)]
pub struct FromState {
    pub foreign_host: String,
    pub from: String,
    pub to: Vec<String>,
}

impl From<&State> for FromState {
    fn from(value: &State) -> Self {
        match &value.state_type {
            StateType::From(v) => v.clone(),
            _ => panic!(),
        }
    }
}

pub async fn handle_from(mut state: State) -> Result<State> {
    let mut from_state: FromState = (&state).into();
    let stream = &mut state.stream;
    let message = rx(stream, false).await?;
    let message = match parse_message(message.clone()) {
        Some(v) => v,
        None => return Err(Error::InvalidCommand(Some(message)).into()),
    };
    let command = message.command;
    match &command as &str {
        "RCPT" => {}
        "DATA" => {
            tx(
                stream,
                "354 Type your message. End with <CR><LF>.<CR><LF>".to_string(),
                false,
                true,
            )
            .await?;
            debug!("Recieving message");
            return Ok(State {
                state_type: StateType::Recieving(RecievingState {
                    foreign_host: from_state.foreign_host,
                    from: from_state.from,
                    to: from_state.to,
                }),
                stream: state.stream,
            });
        }
        _ => return Err(Error::InvalidCommand(Some(command)).into()),
    }

    let to = message.args.join(" ");
    let to = match to.strip_prefix("TO:") {
        Some(v) => v.to_string(),
        None => return Err(Error::InvalidCommand(Some(to)).into()),
    };
    let to = to.trim();
    from_state.to.push(to.to_string());
    let to = to.trim_start_matches('<');
    let to = to.trim_end_matches('>');
    let to = to.to_string();
    debug!("Sending the mail to {to}");
    tx(
        stream,
        format!("250 I will make sure your message gets to {to} :3"),
        false,
        true,
    )
    .await?;

    Ok(State {
        state_type: StateType::From(from_state),
        stream: state.stream,
    })
}
