use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub memory: MemoryConfig,
    pub persistence: PersistenceConfig,
    pub logging: LoggingConfig,
    #[serde(default)]
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub max_memory: String, // e.g., "1GB", "512MB"
    pub eviction_policy: String, // "allkeys-lfu", "allkeys-lru", "noeviction"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    #[serde(default)]
    pub aof_enabled: bool,
    #[serde(default = "default_aof_fsync")]
    pub aof_fsync: String, // "always", "everysec", "no"
    #[serde(default = "default_aof_filename")]
    pub aof_filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String, // "trace", "debug", "info", "warn", "error"
}

/// Security configuration for authentication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Whether authentication is required
    #[serde(default)]
    pub require_auth: bool,
    /// Password for AUTH command (None = no password set)
    #[serde(default)]
    pub password: Option<String>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        SecurityConfig {
            require_auth: false,
            password: None,
        }
    }
}

// Default functions
fn default_max_connections() -> usize {
    10000
}

fn default_metrics_port() -> u16 {
    9090
}

fn default_aof_fsync() -> String {
    "everysec".to_string()
}

fn default_aof_filename() -> String {
    "appendonly.aof".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 7878,
                max_connections: default_max_connections(),
                metrics_port: default_metrics_port(),
            },
            memory: MemoryConfig {
                max_memory: "64MB".to_string(),
                eviction_policy: "allkeys-lfu".to_string(),
            },
            persistence: PersistenceConfig {
                aof_enabled: false,
                aof_fsync: default_aof_fsync(),
                aof_filename: default_aof_filename(),
            },
            logging: LoggingConfig {
                level: default_log_level(),
            },
            security: SecurityConfig::default(),
        }
    }

}

impl Config {
    /// Load configuration from TOML file
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    /// Load configuration from file if it exists, otherwise use defaults
    pub fn load_or_default(path: impl AsRef<Path>) -> Self {
        Config::from_file(path).unwrap_or_else(|_| Config::default())
    }

    /// Save configuration to TOML file
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
        let toml_string = toml::to_string_pretty(self)?;
        std::fs::write(path, toml_string)?;
        Ok(())
    }

    /// Parse max_memory string to bytes (e.g., "1GB" -> 1073741824)
    pub fn max_memory_bytes(&self) -> usize {
        parse_memory_string(&self.memory.max_memory)
    }
}

/// Parse memory strings like "1GB", "512MB", "256KB" to bytes
fn parse_memory_string(s: &str) -> usize {
    let s = s.trim().to_uppercase();
    
    if let Some(bytes_str) = s.strip_suffix("GB") {
        bytes_str.trim().parse::<usize>().unwrap_or(64) * 1024 * 1024 * 1024
    } else if let Some(bytes_str) = s.strip_suffix("MB") {
        bytes_str.trim().parse::<usize>().unwrap_or(64) * 1024 * 1024
    } else if let Some(bytes_str) = s.strip_suffix("KB") {
        bytes_str.trim().parse::<usize>().unwrap_or(64) * 1024
    } else {
        // Assume bytes
        s.parse().unwrap_or(64 * 1024 * 1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_memory_string() {
        assert_eq!(parse_memory_string("1GB"), 1024 * 1024 * 1024);
        assert_eq!(parse_memory_string("512MB"), 512 * 1024 * 1024);
        assert_eq!(parse_memory_string("256KB"), 256 * 1024);
        assert_eq!(parse_memory_string("1024"), 1024);
    }

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 7878);
        assert_eq!(config.logging.level, "info");
    }
}
