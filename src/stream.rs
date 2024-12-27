use std::{
    io::{Cursor, Error, ErrorKind, Result},
    sync::Arc, time::Duration,
};

use color_eyre::eyre::eyre;
use rustls_pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer, ServerName};
use smtp_proto::{response::parser::ResponseReceiver, Request, Response};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream, time::sleep,
};
use tokio_rustls::{TlsAcceptor, TlsStream};
use tracing::debug;

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

        debug!(
            "We are S: S: {}",
            String::from_utf8_lossy(&response_string.clone().into_inner())
        );

        stream.write_all(&response_string.into_inner()).await?;

        Ok(())
    }

    pub async fn recieve_response(&mut self) -> color_eyre::eyre::Result<Response<String>> {
        let stream = *self.deref();

        let mut bufreader = BufReader::new(stream);

        let mut buffer: Vec<u8> = Vec::new();
        let _num_bytes_recieved = bufreader.read_until(b'\n', &mut buffer).await?;

        debug!("We are C: S: {}", String::from_utf8_lossy(&buffer));

        let mut reciever = ResponseReceiver::default();
        let response = reciever.parse(&mut buffer.iter())?;

        Ok(response)
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

            debug!("We are S: C: {}", String::from_utf8_lossy(&buffer));

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

    pub async fn send_request<T: std::fmt::Display>(&mut self, request: Request<T>) -> Result<()> {
        let stream = *self.deref();

        use Request as R;
        let request = match request {
            R::Quit => "QUIT",
            R::Data => "DATA",
            R::StartTls => "STARTTLS",
            R::Ehlo { host } => &format!("EHLO {host}"),
            R::Helo { host } => &format!("HELO {host}"),
            R::Mail { from } => &format!("MAIL FROM:{}", from.address),
            R::Rcpt { to } => &format!("RCPT TO:{}", to.address),
            _ => unimplemented!(),
        };
        let request = request.to_string() + "\r\n";

        debug!("We are C: C: {}", &request);

        stream.write_all(&request.as_bytes()).await?;

        Ok(())
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

    pub async fn send_mail(&mut self, mail: &[u8]) -> Result<()> {
        let stream = *self.deref();

        stream.write_all(mail).await?;

        Ok(())
    }

    pub async fn quit(&mut self) -> Error {
        let _ = self
            .send_response(Response::new(221, 2, 0, 0, "Byebye!"))
            .await;

        Error::new(ErrorKind::ConnectionReset, "Client Quit")
    }

    pub async fn protocol_error(&mut self) -> Result<()> {
        self.send_response(Response::new(500, 5, 5, 0, "Syntax Error"))
            .await
    }

    pub async fn start_tls_server(self) -> Result<Self> {
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
        let stream = TlsStream::Server(stream);

        return Ok(Self::Tls(stream));
    }

    pub async fn start_tls_client(self, server_name: String) -> Result<Self> {
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

        let root_store =
            rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        let rc_config = Arc::new(config);

        let connector = tokio_rustls::TlsConnector::from(rc_config);
        let dns_name = match ServerName::try_from(server_name) {
            Ok(v) => v,
            Err(e) => return Err(Error::new(ErrorKind::InvalidData, format!("{e}"))),
        };

        let stream = connector.connect(dns_name, stream).await?;
        let stream = TlsStream::Client(stream);

        Ok(Self::Tls(stream))
    }

    pub async fn recieve_capabilities(&mut self) -> color_eyre::eyre::Result<Vec<String>> {
        let stream = *self.deref();

        sleep(Duration::from_millis(10)).await;

        let mut buf = Vec::new();
        let _num_recieved_bytes = stream.read_buf(&mut buf);

        let buf = String::from_utf8_lossy(&buf);

        let capabilties = Self::parse_capabilties(buf.to_string())?;

        Ok(capabilties)
    }

    fn parse_capabilties(capabilties: String) -> color_eyre::eyre::Result<Vec<String>> {
        let mut lines = capabilties.split("\r\n");
        lines.next(); // remove first line

        let mut capabilties = Vec::new();

        for line in lines {
            let capability: String;
            let separator = if line.contains('-') { '-' } else { ' ' };

            capability = match line.split(separator).next_back() {
                Some(v) => v.to_string(),
                None => return Err(eyre!(":3")),
            };

            capabilties.push(capability)
        }

        Ok(capabilties)
    }
}
