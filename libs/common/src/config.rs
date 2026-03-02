use std::env;
use std::num::ParseIntError;

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub log_level: String,
    pub database_url: String,
    pub db_max_connections: u32,
    pub rbac_enabled: bool,
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let host = env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = parse_u16_env("API_PORT", 8080)?;
        let log_level = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://cloudops:cloudops_local_change_me@127.0.0.1:5432/cloudops".to_string()
        });
        let db_max_connections = parse_u32_env("DB_MAX_CONNECTIONS", 10)?;
        let rbac_enabled = parse_bool_env("AUTH_RBAC_ENABLED", true)?;

        Ok(Self {
            host,
            port,
            log_level,
            database_url,
            db_max_connections,
            rbac_enabled,
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

fn parse_u32_env(key: &str, default: u32) -> Result<u32, ConfigError> {
    match env::var(key) {
        Ok(value) => value
            .parse::<u32>()
            .map_err(|source| ConfigError::InvalidNumber {
                key: key.to_string(),
                value,
                source,
            }),
        Err(_) => Ok(default),
    }
}

fn parse_bool_env(key: &str, default: bool) -> Result<bool, ConfigError> {
    match env::var(key) {
        Ok(value) => {
            let normalized = value.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "1" | "true" | "yes" | "on" => Ok(true),
                "0" | "false" | "no" | "off" => Ok(false),
                _ => Err(ConfigError::InvalidBool {
                    key: key.to_string(),
                    value,
                }),
            }
        }
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
    #[error("invalid boolean for {key}: {value}")]
    InvalidBool { key: String, value: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_defaults() {
        let cfg = AppConfig::from_env().expect("default config should load");
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.host, "0.0.0.0");
        assert_eq!(cfg.db_max_connections, 10);
        assert!(cfg.rbac_enabled);
        assert_eq!(
            cfg.database_url,
            "postgres://cloudops:cloudops_local_change_me@127.0.0.1:5432/cloudops"
        );
    }
}
