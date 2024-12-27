use std::{
    io::{Cursor, Error, ErrorKind, Result},
    sync::Arc,
};

use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use smtp_proto::{Request, Response};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};
use tokio_rustls::{server::TlsStream, TlsAcceptor};

use crate::AsyncStream;

#[derive(Debug)]
pub enum Stream {
    Tcp(TcpStream),
    Tls(TlsStream<TcpStream>),
}

impl Stream {
    pub fn deref(&mut self) -> Box<&mut dyn AsyncStream> {
        match self {
            Self::Tcp(stream) => Box::new(stream),
            Self::Tls(stream) => Box::new(stream),
        }
    }

    pub async fn send_response<T: std::fmt::Display>(
        &mut self,
        response: Response<T>,
    ) -> Result<()> {
        let stream = *self.deref();
        let mut response_string = Cursor::new(Vec::new());
        response.write(&mut response_string)?;

        stream.write_all(&response_string.into_inner()).await?;

        Ok(())
    }

    pub async fn protocol_error(&mut self) -> Result<()> {
        self.send_response(Response::new(500, 5, 5, 0, "Syntax Error"))
            .await
    }

    pub async fn recieve_mail(&mut self) -> Result<Vec<u8>> {
        let stream = *self.deref();
        let mut buf: Vec<u8> = Vec::new();

        loop {
            buf.push(stream.read_u8().await?);

            if buf.ends_with(b"\r\n.\r\n") {
                break;
            }
        }

        Ok(buf)
    }

    pub async fn recieve_request(&mut self) -> Result<Request<String>> {
        let stream = *self.deref();

        let mut bufreader = BufReader::new(stream);

        loop {
            let mut buffer: Vec<u8> = Vec::new();
            let _num_bytes_recieved = bufreader.read_until(b'\n', &mut buffer).await?;

            if buffer.trim_ascii().is_empty() {
                continue;
            }

            match Request::parse(&mut buffer.iter()) {
                Ok(request) => match request {
                    Request::Quit => return Err(self.quit().await),
                    request => return Ok(request),
                },
                Err(_e) => {
                    bufreader.write_all(b"500 5.5.0 Syntax Error\r\n").await?;
                }
            };
        }
    }

    pub async fn quit(&mut self) -> Error {
        let _ = self
            .send_response(Response::new(221, 2, 0, 0, "Byebye!"))
            .await;

        Error::new(ErrorKind::ConnectionReset, "Client Quit")
    }

    pub async fn start_tls(self) -> Result<Self> {
        let stream = match self {
            Self::Tcp(a) => a,
            Self::Tls(_) => {
                return Err(Error::new(
                    ErrorKind::Unsupported,
                    "tfw någon försöker starttls två gånger",
                )
                .into())
            }
        };

        let certs = CertificateDer::pem_file_iter("cert.pem")
            .unwrap()
            .map(|x| x.unwrap())
            .collect::<Vec<_>>();
        let key = PrivateKeyDer::from_pem_file("privkey.pem").unwrap();

        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .unwrap();
        let acceptor = TlsAcceptor::from(Arc::new(config));

        let stream = acceptor.accept(stream).await?;

        return Ok(Self::Tls(stream));
    }
}
