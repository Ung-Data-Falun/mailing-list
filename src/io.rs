use std::time::Duration;

use color_eyre::eyre::Result;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    time::sleep,
};
use tracing::debug;

pub async fn tx(stream: &mut (impl AsyncWrite + std::marker::Unpin), msg: String) -> Result<()> {
    stream.write_all(msg.as_bytes()).await?;
    stream.write_u8(b'\r').await?;
    stream.write_u8(b'\n').await?;
    stream.flush().await?;
    debug!("S: {:?}", msg);

    Ok(())
}

pub async fn rx(stream: &mut (impl AsyncRead + std::marker::Unpin)) -> Result<String> {
    sleep(Duration::from_millis(10)).await;
    let buf = &mut [0; 1000];
    stream.read(buf).await?;
    let buf = String::from_utf8(buf.to_vec())?
        .trim_end_matches(['\0'])
        .to_string();
    debug!("C: {:?}", buf);

    Ok(buf.to_string())
}
