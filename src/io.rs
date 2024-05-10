use std::time::Duration;

use color_eyre::eyre::Result;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    time::sleep,
};
use tracing::debug;

pub async fn tx(
    stream: &mut (impl AsyncWrite + std::marker::Unpin),
    msg: String,
    obfuscate: bool,
    add_newline: bool,
) -> Result<()> {
    let mut msg = msg;
    if add_newline {
        msg = msg + "\r\n";
    }
    stream.write_all(msg.as_bytes()).await?;
    stream.flush().await?;
    if !obfuscate {
        debug!("TX: {:?}", msg);
    } else {
        debug!("TX: <obfuscated data>");
    }

    Ok(())
}

pub async fn rx(
    stream: &mut (impl AsyncRead + std::marker::Unpin),
    obfuscate: bool,
) -> Result<String> {
    sleep(Duration::from_millis(10)).await;
    let buf = &mut [0; 1000];
    stream.read(buf).await?;
    let buf = String::from_utf8(buf.to_vec())?
        .trim_end_matches(['\0'])
        .to_string();
    if !obfuscate {
        debug!("RX: {:?}", buf);
    } else {
        debug!("RX: <obfuscated data>")
    }

    Ok(buf.to_string())
}
