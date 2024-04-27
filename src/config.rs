use color_eyre::eyre::Result;
use serde::Deserialize;
use std::{collections::HashMap, path::Path};

#[derive(Deserialize, Debug, Clone)]
pub struct ServerConfig {
    pub port: Option<u16>,
    pub ip: Option<String>,
    pub lists: HashMap<String, List>,
    pub forwarding: Option<ForwardingOptions>,
    pub hostname: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ForwardingOptions {
    pub enable: bool,
    pub server: Option<String>,
    pub server_tls: Option<String>,
    pub port: Option<u16>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct List {
    pub members: Vec<String>,
}

pub fn get_config(file: Option<&str>) -> Result<ServerConfig> {
    let file = &Path::new(file.unwrap_or("/etc/mailing-list/daemon.toml"));
    let file_contents = String::from_utf8(std::fs::read(file)?)?;
    return Ok(toml::from_str(&file_contents)?);
}
