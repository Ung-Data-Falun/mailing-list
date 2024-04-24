use color_eyre::eyre::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize, Debug, Clone)]
pub struct ServerConfig {
    pub port: Option<u16>,
    pub ip: Option<String>,
    pub hostname: String,
}

pub fn get_config(file: Option<&str>) -> Result<ServerConfig> {
    let file = &Path::new(file.unwrap_or("/etc/mailing-list/daemon.conf"));
    let file_contents = String::from_utf8(std::fs::read(file)?)?;
    return Ok(toml::from_str(&file_contents)?);
}
