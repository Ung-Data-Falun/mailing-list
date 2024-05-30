use color_eyre::eyre::Result;

use crate::{
    client_handler::{parse_message, State, StateType},
    error::Error,
    io::{rx, tx},
};

use super::idle::IdleState;

#[derive(Debug, Clone)]
pub struct ConnectedState;

pub async fn handle_connected(mut state: State) -> Result<State> {
    let stream = &mut state.stream;
    let message = rx(stream, false).await?;
    let message = match parse_message(message.clone()) {
        Some(v) => v,
        None => return Err(Error::InvalidCommand(Some(message)).into()),
    };
    let command = message.command.to_uppercase();
    match &command as &str {
        "HELO" => {}
        "EHLO" => {}
        _ => {
            return Err(Error::InvalidCommand(Some(command)).into());
        }
    };

    let foreign_host = match message.args.get(0) {
        Some(v) => v.to_string(),
        None => return Err(Error::InvalidCommand(Some(command)).into()),
    };

    tx(
        stream,
        format!(
            "250 Hello {}, nice to meet you. I'm running mailing-list. Any questions? :)",
            foreign_host
        ),
        false,
        true,
    )
    .await?;

    Ok(State {
        state_type: StateType::Idle(IdleState { foreign_host }),
        stream: state.stream,
    })
}
