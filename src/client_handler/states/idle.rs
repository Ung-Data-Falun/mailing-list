use color_eyre::eyre::Result;
use tracing::debug;

use crate::{
    client_handler::{parse_message, states::from::FromState, State},
    error::Error,
    io::{rx, tx},
    AsyncStream,
};

#[derive(Debug, Clone)]
pub struct IdleState {
    pub foreign_host: String,
}

pub async fn handle_idle(stream: &mut impl AsyncStream, state: IdleState) -> Result<State> {
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
                format!("221 Bye bye {}. Nice to talk to you :3", state.foreign_host),
                false,
                true,
            )
            .await?;
            return Err(Error::Quit.into());
        }
        "HELO" => {
            tx(
                stream,
                format!("250 Hello {}", state.foreign_host),
                false,
                true,
            )
            .await?;
            return Ok(State::Idle(state));
        }
        "EHLO" => {
            tx(
                stream,
                format!("250 Hello {}", state.foreign_host),
                false,
                true,
            )
            .await?;
            return Ok(State::Idle(state));
        }
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

    Ok(State::From(FromState {
        foreign_host: state.foreign_host,
        from,
        to: Vec::new(),
    }))
}
