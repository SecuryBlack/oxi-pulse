use serde::Deserialize;
use std::{env, fs, path::Path};

#[derive(Debug, Deserialize)]
pub struct Config {
    /// OTLP collector endpoint (e.g. "https://ingest.example.com:4317")
    pub endpoint: String,
    /// Authentication token sent as a header to the OTLP collector
    pub token: String,
    /// How often to collect and send metrics, in seconds (default: 10)
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
    /// Maximum number of metric snapshots to buffer when offline (default: 8640 = 24h at 10s)
    #[serde(default = "default_buffer_max")]
    pub buffer_max_size: usize,
    /// Opt-in usage telemetry sent to SecuryBlack.
    /// - absent / not set → defers to server-side config (fetched on startup)
    /// - true             → always enabled, ignores server-side config
    /// - false            → always disabled, ignores server-side config
    pub telemetry_enabled: Option<bool>,
    /// SecuryBlack API base URL used for remote config and telemetry pings.
    /// Defaults to "https://api.securyblack.com".
    #[serde(default = "default_api_url")]
    pub api_url: String,
}

fn default_interval() -> u64 {
    10
}

fn default_buffer_max() -> usize {
    8640
}

fn default_api_url() -> String {
    "https://api.securyblack.com".to_string()
}

#[derive(Debug)]
pub enum ConfigError {
    MissingEndpoint,
    MissingToken,
    ParseError(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::MissingEndpoint => write!(
                f,
                "missing OTLP endpoint — set 'endpoint' in config.toml or OXIPULSE_ENDPOINT env var"
            ),
            ConfigError::MissingToken => write!(
                f,
                "missing auth token — set 'token' in config.toml or OXIPULSE_TOKEN env var"
            ),
            ConfigError::ParseError(msg) => write!(f, "config parse error: {}", msg),
        }
    }
}

impl Config {
    /// Load config from `config.toml` (if present), then override with env vars.
    /// Fails with a clear error if required fields are missing.
    pub fn load() -> Result<Self, ConfigError> {
        // Start with values from config.toml if it exists
        let mut endpoint: Option<String> = None;
        let mut token: Option<String> = None;
        let mut interval_secs: u64 = default_interval();
        let mut buffer_max_size: usize = default_buffer_max();
        let mut telemetry_enabled: Option<bool> = None;
        let mut api_url: String = default_api_url();

        let config_path = Self::config_file_path();
        if Path::new(&config_path).exists() {
            let contents = fs::read_to_string(&config_path)
                .map_err(|e| ConfigError::ParseError(e.to_string()))?;
            let file: toml::Value = toml::from_str(&contents)
                .map_err(|e| ConfigError::ParseError(e.to_string()))?;

            if let Some(v) = file.get("endpoint").and_then(|v| v.as_str()) {
                endpoint = Some(v.to_string());
            }
            if let Some(v) = file.get("token").and_then(|v| v.as_str()) {
                token = Some(v.to_string());
            }
            if let Some(v) = file.get("interval_secs").and_then(|v| v.as_integer()) {
                interval_secs = v as u64;
            }
            if let Some(v) = file.get("buffer_max_size").and_then(|v| v.as_integer()) {
                buffer_max_size = v as usize;
            }
            if let Some(v) = file.get("telemetry_enabled").and_then(|v| v.as_bool()) {
                telemetry_enabled = Some(v);
            }
            if let Some(v) = file.get("api_url").and_then(|v| v.as_str()) {
                api_url = v.to_string();
            }
        }

        // Env vars override config file
        if let Ok(v) = env::var("OXIPULSE_ENDPOINT") {
            endpoint = Some(v);
        }
        if let Ok(v) = env::var("OXIPULSE_TOKEN") {
            token = Some(v);
        }
        if let Ok(v) = env::var("OXIPULSE_INTERVAL_SECS") {
            if let Ok(n) = v.parse::<u64>() {
                interval_secs = n;
            }
        }
        if let Ok(v) = env::var("OXIPULSE_BUFFER_MAX") {
            if let Ok(n) = v.parse::<usize>() {
                buffer_max_size = n;
            }
        }
        if let Ok(v) = env::var("OXIPULSE_TELEMETRY") {
            match v.to_lowercase().as_str() {
                "true" | "1" | "yes" => telemetry_enabled = Some(true),
                "false" | "0" | "no" => telemetry_enabled = Some(false),
                _ => {}
            }
        }
        if let Ok(v) = env::var("OXIPULSE_API_URL") {
            api_url = v;
        }

        Ok(Config {
            endpoint: endpoint.ok_or(ConfigError::MissingEndpoint)?,
            token: token.ok_or(ConfigError::MissingToken)?,
            interval_secs,
            buffer_max_size,
            telemetry_enabled,
            api_url,
        })
    }

    fn config_file_path() -> String {
        #[cfg(target_os = "windows")]
        return r"C:\ProgramData\oxipulse\config.toml".to_string();

        #[cfg(not(target_os = "windows"))]
        return "/etc/oxipulse/config.toml".to_string();
    }
}
