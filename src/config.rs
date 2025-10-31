use serde::Deserialize;
use std::fs;

#[derive(Debug, Deserialize, Clone)]
pub struct McpServerConfig {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub mcp_servers: Vec<McpServerConfig>,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self, anyhow::Error> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }
}
