use kursal_core::{KursalError, Result};
use libp2p::Multiaddr;
use serde::Deserialize;
use std::{net::SocketAddr, path::Path};

#[derive(Deserialize)]
pub struct RelayConfigFile {
    pub listen_addr: String,
    pub announce_addr: String,
    pub max_connections: usize,
    pub max_connections_per_ip: Option<usize>,
    pub log_file: Option<String>,
    pub log_level: String,
    pub bootstrap_peers: Vec<String>,
    pub health: HealthConfig,
}

pub struct RelayConfig {
    pub listen_addr: SocketAddr,
    pub announce_addr: SocketAddr,
    pub max_connections: usize,
    pub max_connections_per_ip: usize,
    pub log_file: Option<String>,
    pub log_level: String,
    pub bootstrap_peers: Vec<Multiaddr>,
    pub health: HealthConfig,
}

impl RelayConfig {
    pub fn load(path: &Path) -> Result<RelayConfig> {
        let config_content = std::fs::read(path).map_err(KursalError::Io)?;
        let content: RelayConfigFile = toml::from_slice(&config_content)
            .map_err(|err| KursalError::Storage(err.to_string()))?;

        Ok(RelayConfig {
            listen_addr: content.listen_addr.parse::<SocketAddr>().map_err(|err| {
                KursalError::Storage(format!("Could not parse listen_addr: {err}"))
            })?,
            announce_addr: content.announce_addr.parse::<SocketAddr>().map_err(|err| {
                KursalError::Storage(format!("Could not parse announce_addr: {err}"))
            })?,
            max_connections: content.max_connections,
            max_connections_per_ip: content.max_connections_per_ip.unwrap_or(3usize),
            log_file: content.log_file,
            log_level: content.log_level,
            bootstrap_peers: content
                .bootstrap_peers
                .into_iter()
                .map(|el| el.parse::<Multiaddr>())
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|err| KursalError::Storage(err.to_string()))?,
            health: content.health,
        })
    }
}

#[derive(Deserialize)]
pub struct HealthConfig {
    pub enabled: bool,
    pub listen_addr: String,
}
