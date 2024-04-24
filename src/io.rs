use color_eyre::eyre::Result;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufStream},
    net::TcpStream,
};
use tracing::info;

pub async fn tx(stream: &mut BufStream<TcpStream>, msg: String) -> Result<()> {
    stream.write_all(msg.as_bytes()).await?;
    stream.write_u8(b'\r').await?;
    stream.write_u8(b'\n').await?;
    stream.flush().await?;
    info!("S: {}", msg);

    Ok(())
}

pub async fn rx(stream: &mut BufStream<TcpStream>) -> Result<String> {
    let mut buf = String::new();
    stream.read_line(&mut buf).await?;
    let buf = buf.trim_end().to_string();
    info!("C: {}", buf);

    Ok(buf)
}
