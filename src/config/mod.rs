use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;

#[derive(Debug, Deserialize)]
pub struct Config {
    /// Agent version
    #[serde(default)]
    pub version: Option<String>,
    /// OTLP collector endpoint (e.g. "https://ingest.example.com:4317")
    #[serde(default)]
    pub endpoint: String,
    /// Authentication token sent as a Bearer header to the OTLP collector.
    /// Optional: leave empty for collectors that need no auth.
    #[serde(default)]
    pub token: String,
    /// Extra gRPC headers sent with every export, for backends that want their
    /// own auth or routing headers (e.g. `signoz-ingestion-key`, `uptrace-dsn`).
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
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
    /// - absent / not set → off, unless the endpoint is SecuryBlack Cloud, in
    ///   which case the account's server-side setting decides
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
    /// Check GitHub Releases daily and self-update (default: true).
    #[serde(default = "default_auto_update")]
    pub auto_update: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: None,
            endpoint: String::new(),
            token: String::new(),
            headers: BTreeMap::new(),
            mode: None,
            interval_secs: default_interval(),
            buffer_max_size: default_buffer_max(),
            telemetry_enabled: None,
            api_url: default_api_url(),
            latency_targets: Vec::new(),
            auto_update: default_auto_update(),
        }
    }
}

fn default_interval() -> u64 {
    30
}

fn default_buffer_max() -> usize {
    8640
}

fn default_auto_update() -> bool {
    true
}

fn default_api_url() -> String {
    "https://api.securyblack.com".to_string()
}

/// Parses `key1=value1,key2=value2`, the format of OTEL_EXPORTER_OTLP_HEADERS.
/// Values may contain `=` (Basic auth, DSNs); only the first one splits.
fn parse_headers(s: &str) -> BTreeMap<String, String> {
    s.split(',')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.trim().to_lowercase(), v.trim().to_string()))
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

#[derive(Debug)]
pub enum ConfigError {
    MissingEndpoint,
    ParseError(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::MissingEndpoint => write!(
                f,
                "missing OTLP endpoint — set 'endpoint' in config.toml or OXIPULSE_ENDPOINT env var"
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
        if let Ok(v) = env::var("OXIPULSE_HEADERS") {
            cfg.headers = parse_headers(&v);
        }
        if let Ok(v) = env::var("OXIPULSE_INTERVAL_SECS")
            && let Ok(n) = v.parse::<u64>()
        {
            cfg.interval_secs = n;
        }
        if let Ok(v) = env::var("OXIPULSE_BUFFER_MAX")
            && let Ok(n) = v.parse::<usize>()
        {
            cfg.buffer_max_size = n;
        }
        if let Ok(v) = env::var("OXIPULSE_TELEMETRY") {
            match v.to_lowercase().as_str() {
                "true" | "1" | "yes" => cfg.telemetry_enabled = Some(true),
                "false" | "0" | "no" => cfg.telemetry_enabled = Some(false),
                _ => {}
            }
        }
        if let Ok(v) = env::var("OXIPULSE_AUTO_UPDATE") {
            match v.to_lowercase().as_str() {
                "true" | "1" | "yes" => cfg.auto_update = true,
                "false" | "0" | "no" => cfg.auto_update = false,
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

        let current_pkg_version = env!("CARGO_PKG_VERSION");
        cfg.version = Some(current_pkg_version.to_string());
        let _ = sb_agent_core::config::sync_version_field(&config_path, current_pkg_version);

        Ok(cfg)
    }
}

impl Config {
    /// True when metrics go to SecuryBlack Cloud. Only then may the agent talk
    /// to the SecuryBlack API with `token`: for any other backend the token is
    /// a third-party credential and must never leave towards SecuryBlack.
    pub fn is_securyblack_endpoint(&self) -> bool {
        let rest = self
            .endpoint
            .split_once("://")
            .map_or(self.endpoint.as_str(), |(_, r)| r);
        let host = rest.split(['/', ':']).next().unwrap_or("");
        host == "securyblack.com" || host.ends_with(".securyblack.com")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error_display() {
        assert!(format!("{}", ConfigError::MissingEndpoint).contains("missing OTLP endpoint"));
        assert!(
            format!("{}", ConfigError::ParseError("invalid toml".to_string()))
                .contains("invalid toml")
        );
    }

    #[test]
    fn test_parse_headers() {
        let h = parse_headers("Authorization=Basic YTpi==, stream-name=default,bad");
        assert_eq!(h.len(), 2);
        assert_eq!(h["authorization"], "Basic YTpi==");
        assert_eq!(h["stream-name"], "default");
    }

    #[test]
    fn test_is_securyblack_endpoint() {
        let with = |e: &str| Config {
            endpoint: e.to_string(),
            ..Config::default()
        };
        assert!(with("https://ingest.securyblack.com:4317").is_securyblack_endpoint());
        assert!(with("ingest.securyblack.com").is_securyblack_endpoint());
        assert!(!with("https://otlp.grafana.net:443").is_securyblack_endpoint());
        assert!(!with("http://localhost:4317").is_securyblack_endpoint());
        assert!(!with("https://securyblack.com.evil.io").is_securyblack_endpoint());
        assert!(!with("https://notsecuryblack.com").is_securyblack_endpoint());
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
