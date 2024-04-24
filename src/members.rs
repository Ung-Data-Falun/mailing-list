use std::path::Path;

use color_eyre::eyre::Result;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Members {
    pub members: Vec<String>,
}

pub fn get_members(location: Option<&str>) -> Result<Members> {
    let path = &Path::new(location.unwrap_or("/etc/mailing-list/members"));
    let data = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&data)?)
}
