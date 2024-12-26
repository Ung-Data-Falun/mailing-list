use color_eyre::eyre::Result;
use serde::Deserialize;
use std::{collections::HashMap, path::Path};

#[derive(Deserialize, Debug, Clone)]
pub struct ServerConfig {
    pub port: Option<u16>,
    pub ip: Option<String>,
    pub lists: HashMap<String, List>,
    pub forwarding: Option<ForwardingOptions>,
    pub plugins: Vec<String>,
    pub hostname: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ForwardingOptions {
    pub enable: bool,
    pub server: Option<String>,
    pub server_tls: String,
    pub port: Option<u16>,
}

#[derive(Deserialize, Debug, Clone)]
pub enum List {
    Local(LocalList),
    Remote(RemoteList),
}

#[derive(Deserialize, Debug, Clone)]
pub struct LocalList {
    pub members: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RemoteList {
    pub location: String,
}

#[derive(Deserialize, Debug, Clone)]
struct MedlemsLista {
    pub medlemmar: Vec<Medlem>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
struct Medlem {
    namn: String,
    mail: String,
}

impl List {
    pub async fn get_members(&self) -> Result<Vec<String>> {
        Ok(match self.clone() {
            Self::Local(list) => list.members,
            Self::Remote(list) => {
                let lista: MedlemsLista =
                    toml::from_str(&tokio::fs::read_to_string(list.location).await?)?;
                lista
                    .medlemmar
                    .iter()
                    .map(|x| format!("<{}>", x.mail))
                    .collect()
            }
        })
    }
}

pub fn get_config(file: Option<&str>) -> Result<ServerConfig> {
    let file = &Path::new(file.unwrap_or("/etc/mailing-list/daemon.toml"));
    let file_contents = String::from_utf8(std::fs::read(file)?)?;
    return Ok(toml::from_str(&file_contents)?);
}
