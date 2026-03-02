use std::env;
use std::num::ParseIntError;

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub log_level: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let host = env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = parse_u16_env("API_PORT", 8080)?;
        let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

        Ok(Self {
            host,
            port,
            log_level,
        })
    }

    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

fn parse_u16_env(key: &str, default: u16) -> Result<u16, ConfigError> {
    match env::var(key) {
        Ok(value) => value
            .parse::<u16>()
            .map_err(|source| ConfigError::InvalidNumber {
                key: key.to_string(),
                value,
                source,
            }),
        Err(_) => Ok(default),
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid number for {key}: {value}")]
    InvalidNumber {
        key: String,
        value: String,
        source: ParseIntError,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_defaults() {
        let cfg = AppConfig::from_env().expect("default config should load");
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.host, "0.0.0.0");
    }
}
