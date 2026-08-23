use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize)]
pub struct Config {
    /// Agent version
    #[serde(default)]
    pub version: Option<String>,
    /// OTLP collector endpoint (e.g. "https://ingest.example.com:4317")
    #[serde(default)]
    pub endpoint: String,
    /// Authentication token sent as a header to the OTLP collector
    #[serde(default)]
    pub token: String,
    /// Deployment mode: "direct" (default) or "local_agent" (sends to localhost:4317)
    #[serde(default)]
    pub mode: Option<String>,
    /// How often to collect and send metrics, in seconds (default: 30)
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
    /// Maximum number of metric snapshots to buffer when offline (default: 8640 = 72h at 30s)
    #[serde(default = "default_buffer_max")]
    pub buffer_max_size: usize,
    /// Opt-in usage telemetry sent to SecuryBlack.
    /// - absent / not set → defers to server-side config (fetched on startup)
    /// - true             → always enabled, ignores server-side config
    /// - false            → always disabled, ignores server-side config
    #[serde(default)]
    pub telemetry_enabled: Option<bool>,
    /// SecuryBlack API base URL used for remote config and telemetry pings.
    /// Defaults to "https://api.securyblack.com".
    #[serde(default = "default_api_url")]
    pub api_url: String,
    /// Optional list of latency targets to check. Defaults to empty (which falls back to the endpoint).
    #[serde(default)]
    pub latency_targets: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: None,
            endpoint: String::new(),
            token: String::new(),
            mode: None,
            interval_secs: default_interval(),
            buffer_max_size: default_buffer_max(),
            telemetry_enabled: None,
            api_url: default_api_url(),
            latency_targets: Vec::new(),
        }
    }
}

fn default_interval() -> u64 {
    30
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
    /// Carga `config.toml` (vía `sb_agent_core::config::load`), aplica
    /// overrides de entorno y valida los campos obligatorios. La carga en sí
    /// y la sincronización del campo `version` vienen del crate compartido;
    /// lo que sigue siendo propio de OxiPulse son estos campos, sus
    /// `OXIPULSE_*` env vars, y las reglas de `local_agent` / validación.
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = sb_agent_core::config::default_config_path("oxipulse");
        let mut cfg: Config = sb_agent_core::config::load(&config_path)
            .map_err(|e| ConfigError::ParseError(e.to_string()))?;

        if let Ok(v) = env::var("OXIPULSE_ENDPOINT") {
            cfg.endpoint = v;
        }
        if let Ok(v) = env::var("OXIPULSE_TOKEN") {
            cfg.token = v;
        }
        if let Ok(v) = env::var("OXIPULSE_INTERVAL_SECS") {
            if let Ok(n) = v.parse::<u64>() {
                cfg.interval_secs = n;
            }
        }
        if let Ok(v) = env::var("OXIPULSE_BUFFER_MAX") {
            if let Ok(n) = v.parse::<usize>() {
                cfg.buffer_max_size = n;
            }
        }
        if let Ok(v) = env::var("OXIPULSE_TELEMETRY") {
            match v.to_lowercase().as_str() {
                "true" | "1" | "yes" => cfg.telemetry_enabled = Some(true),
                "false" | "0" | "no" => cfg.telemetry_enabled = Some(false),
                _ => {}
            }
        }
        if let Ok(v) = env::var("OXIPULSE_API_URL") {
            cfg.api_url = v;
        }
        if let Ok(v) = env::var("OXIPULSE_MODE") {
            cfg.mode = Some(v);
        }
        if let Ok(v) = env::var("OXIPULSE_LATENCY_TARGETS") {
            cfg.latency_targets = v
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        // When in local_agent mode, default endpoint to localhost:4317
        if cfg.mode.as_deref() == Some("local_agent") && cfg.endpoint.is_empty() {
            cfg.endpoint = "http://localhost:4317".to_string();
        }

        if cfg.endpoint.is_empty() {
            return Err(ConfigError::MissingEndpoint);
        }
        if cfg.token.is_empty() {
            return Err(ConfigError::MissingToken);
        }

        let current_pkg_version = env!("CARGO_PKG_VERSION");
        cfg.version = Some(current_pkg_version.to_string());
        let _ = sb_agent_core::config::sync_version_field(&config_path, current_pkg_version);

        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error_display() {
        assert!(format!("{}", ConfigError::MissingEndpoint).contains("missing OTLP endpoint"));
        assert!(format!("{}", ConfigError::MissingToken).contains("missing auth token"));
        assert!(format!("{}", ConfigError::ParseError("invalid toml".to_string())).contains("invalid toml"));
    }

    #[test]
    fn test_local_agent_mode_default_endpoint() {
        let mode = Some("local_agent".to_string());
        let endpoint = String::new();
        let effective_endpoint = if mode.as_deref() == Some("local_agent") && endpoint.is_empty() {
            "http://localhost:4317".to_string()
        } else {
            endpoint
        };
        assert_eq!(effective_endpoint, "http://localhost:4317".to_string());
    }
}
