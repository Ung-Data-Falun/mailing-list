use color_eyre::eyre::Result;
use tracing::debug;

use crate::{
    client_handler::{parse_message, states::recieving::RecievingState, State},
    error::Error,
    io::{rx, tx},
    AsyncStream,
};

#[derive(Debug, Clone)]
pub struct FromState {
    pub foreign_host: String,
    pub from: String,
    pub to: Vec<String>,
}

pub async fn handle_from(stream: &mut impl AsyncStream, mut state: FromState) -> Result<State> {
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
            return Ok(State::Recieving(RecievingState {
                foreign_host: state.foreign_host,
                from: state.from,
                to: state.to,
            }));
        }
        _ => return Err(Error::InvalidCommand(Some(command)).into()),
    }

    let to = message.args.join(" ");
    let to = match to.strip_prefix("TO:") {
        Some(v) => v.to_string(),
        None => return Err(Error::InvalidCommand(Some(to)).into()),
    };
    let to = to.trim();
    state.to.push(to.to_string());
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

    Ok(State::From(state))
}
