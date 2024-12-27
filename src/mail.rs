use tracing::info;

use crate::{
    config::ServerConfig,
    send_mail::{self, send_group},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mail {
    pub sender: String,
    pub recipients: Vec<String>,
    pub data: String,
}

pub enum Error {
    SendError,
}

type Result<T> = std::result::Result<T, Error>;

impl Mail {
    pub async fn handle(mut self, config: &ServerConfig) -> Result<()> {
        let lists = &config.lists;
        let forwarding_enabled = config.forwarding.clone().is_some_and(|x| x.enable);

        self.sender = format!("<{}>", self.sender);

        for recipient in self.recipients {
            if lists.contains_key(&recipient) {
                info!("Sending to everyone subscribing to {recipient}");
                send_group(
                    &config.hostname,
                    self.data.clone(),
                    &config
                        .lists
                        .get(&recipient)
                        .unwrap()
                        .get_members()
                        .await
                        .unwrap(),
                    &self.sender,
                )
                .await;
            }

            if !forwarding_enabled {
                continue;
            }

            let forwarding = config.forwarding.clone().unwrap();

            if !recipient.ends_with(&forwarding.server_tls) {
                continue;
            }

            let server = forwarding.server.unwrap_or(forwarding.server_tls.clone());

            match send_mail::send(
                &config.hostname,
                &self.data,
                &recipient,
                &self.sender,
                server,
                forwarding.port,
                forwarding.server_tls,
            )
            .await
            {
                Ok(_) => (),
                Err(_) => return Err(Error::SendError),
            };
        }

        Ok(())
    }
}
