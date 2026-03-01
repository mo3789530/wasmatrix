use crate::features::production_config::repo::ConfigurationRepository;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    Development,
    Production,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsConfig {
    pub enabled: bool,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
    pub mtls_required: bool,
    pub mtls_ca_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthConfig {
    pub jwt_secret_configured: bool,
    pub mtls_principals_configured: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapConfig {
    pub runtime_mode: RuntimeMode,
    pub use_etcd: bool,
    pub control_plane_addr: SocketAddr,
    pub metrics_addr: SocketAddr,
    pub rest_api_addr: SocketAddr,
    pub node_id: String,
    pub leader_election_ttl: Duration,
    pub leader_election_renew_interval: Duration,
    pub auth: AuthConfig,
    pub tls: TlsConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapConfigError {
    InvalidValue(String),
    MissingRequiredConfig(String),
}

impl std::fmt::Display for BootstrapConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BootstrapConfigError::InvalidValue(message) => write!(f, "{message}"),
            BootstrapConfigError::MissingRequiredConfig(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for BootstrapConfigError {}

pub struct ProductionConfigService {
    repo: Box<dyn ConfigurationRepository>,
}

impl ProductionConfigService {
    pub fn new(repo: Box<dyn ConfigurationRepository>) -> Self {
        Self { repo }
    }

    pub fn load(&self) -> Result<BootstrapConfig, BootstrapConfigError> {
        let runtime_mode = parse_runtime_mode(self.repo.get("CONTROL_PLANE_MODE").as_deref())?;
        let use_etcd = parse_bool(self.repo.get("USE_ETCD").as_deref(), false)?;

        let control_plane_addr = parse_socket_addr(
            self.repo.get("CONTROL_PLANE_ADDR").as_deref(),
            "127.0.0.1:50051",
            "CONTROL_PLANE_ADDR",
        )?;
        let metrics_addr = parse_socket_addr(
            self.repo.get("METRICS_ADDR").as_deref(),
            "127.0.0.1:9100",
            "METRICS_ADDR",
        )?;
        let rest_api_addr = parse_socket_addr(
            self.repo.get("REST_API_ADDR").as_deref(),
            "127.0.0.1:8080",
            "REST_API_ADDR",
        )?;

        let node_id = self
            .repo
            .get("CONTROL_PLANE_NODE_ID")
            .unwrap_or_else(|| "control-plane-1".to_string());

        let leader_election_ttl = Duration::from_secs(parse_u64(
            self.repo.get("LEADER_ELECTION_TTL_SECS").as_deref(),
            10,
            "LEADER_ELECTION_TTL_SECS",
        )?);
        let leader_election_renew_interval = Duration::from_millis(parse_u64(
            self.repo
                .get("LEADER_ELECTION_RENEW_INTERVAL_MS")
                .as_deref(),
            3_000,
            "LEADER_ELECTION_RENEW_INTERVAL_MS",
        )?);

        if leader_election_renew_interval >= leader_election_ttl {
            return Err(BootstrapConfigError::InvalidValue(
                "LEADER_ELECTION_RENEW_INTERVAL_MS must be smaller than LEADER_ELECTION_TTL_SECS * 1000".to_string(),
            ));
        }

        let auth = AuthConfig {
            jwt_secret_configured: self.repo.get("EXTERNAL_API_JWT_SECRET").is_some(),
            mtls_principals_configured: self.repo.get("EXTERNAL_API_MTLS_PRINCIPALS").is_some(),
        };

        let tls_enabled = parse_bool(self.repo.get("REST_API_TLS_ENABLED").as_deref(), false)?;
        let mtls_required =
            parse_bool(self.repo.get("EXTERNAL_API_REQUIRE_MTLS").as_deref(), false)?;
        let tls = TlsConfig {
            enabled: tls_enabled,
            cert_path: self.repo.get("REST_API_TLS_CERT_PATH"),
            key_path: self.repo.get("REST_API_TLS_KEY_PATH"),
            mtls_required,
            mtls_ca_path: self.repo.get("EXTERNAL_API_MTLS_CA_PATH"),
        };

        if runtime_mode == RuntimeMode::Production {
            if !use_etcd {
                return Err(BootstrapConfigError::MissingRequiredConfig(
                    "Production mode requires USE_ETCD=true".to_string(),
                ));
            }
            if !auth.jwt_secret_configured && !auth.mtls_principals_configured {
                return Err(BootstrapConfigError::MissingRequiredConfig(
                    "Production mode requires EXTERNAL_API_JWT_SECRET or EXTERNAL_API_MTLS_PRINCIPALS".to_string(),
                ));
            }
            if tls.enabled && (tls.cert_path.is_none() || tls.key_path.is_none()) {
                return Err(BootstrapConfigError::MissingRequiredConfig(
                    "REST_API_TLS_ENABLED=true requires REST_API_TLS_CERT_PATH and REST_API_TLS_KEY_PATH".to_string(),
                ));
            }
            if tls.mtls_required && tls.mtls_ca_path.is_none() {
                return Err(BootstrapConfigError::MissingRequiredConfig(
                    "EXTERNAL_API_REQUIRE_MTLS=true requires EXTERNAL_API_MTLS_CA_PATH".to_string(),
                ));
            }
        }

        Ok(BootstrapConfig {
            runtime_mode,
            use_etcd,
            control_plane_addr,
            metrics_addr,
            rest_api_addr,
            node_id,
            leader_election_ttl,
            leader_election_renew_interval,
            auth,
            tls,
        })
    }
}

fn parse_runtime_mode(raw: Option<&str>) -> Result<RuntimeMode, BootstrapConfigError> {
    match raw
        .unwrap_or("development")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "development" | "dev" => Ok(RuntimeMode::Development),
        "production" | "prod" => Ok(RuntimeMode::Production),
        value => Err(BootstrapConfigError::InvalidValue(format!(
            "CONTROL_PLANE_MODE must be development|production, got '{value}'"
        ))),
    }
}

fn parse_bool(raw: Option<&str>, default: bool) -> Result<bool, BootstrapConfigError> {
    match raw {
        None => Ok(default),
        Some(value) => match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            _ => Err(BootstrapConfigError::InvalidValue(format!(
                "Invalid boolean value '{value}'"
            ))),
        },
    }
}

fn parse_u64(raw: Option<&str>, default: u64, key: &str) -> Result<u64, BootstrapConfigError> {
    let value = raw.unwrap_or_default();
    if value.is_empty() {
        return Ok(default);
    }
    value.parse::<u64>().map_err(|_| {
        BootstrapConfigError::InvalidValue(format!("{key} must be an unsigned integer"))
    })
}

fn parse_socket_addr(
    raw: Option<&str>,
    default: &str,
    key: &str,
) -> Result<SocketAddr, BootstrapConfigError> {
    let value = raw.unwrap_or(default);
    SocketAddr::from_str(value).map_err(|_| {
        BootstrapConfigError::InvalidValue(format!("{key} is not a valid socket address"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::production_config::repo::InMemoryConfigurationRepository;

    #[test]
    fn defaults_to_development_mode() {
        let service =
            ProductionConfigService::new(Box::new(InMemoryConfigurationRepository::default()));
        let config = service.load().unwrap();
        assert_eq!(config.runtime_mode, RuntimeMode::Development);
        assert!(!config.use_etcd);
    }

    #[test]
    fn rejects_production_without_etcd() {
        let repo =
            InMemoryConfigurationRepository::default().with("CONTROL_PLANE_MODE", "production");
        let service = ProductionConfigService::new(Box::new(repo));
        let error = service.load().unwrap_err();
        assert!(matches!(
            error,
            BootstrapConfigError::MissingRequiredConfig(_)
        ));
    }

    #[test]
    fn accepts_production_with_etcd_and_authn() {
        let repo = InMemoryConfigurationRepository::default()
            .with("CONTROL_PLANE_MODE", "production")
            .with("USE_ETCD", "true")
            .with("EXTERNAL_API_JWT_SECRET", "secret");
        let service = ProductionConfigService::new(Box::new(repo));
        let config = service.load().unwrap();
        assert_eq!(config.runtime_mode, RuntimeMode::Production);
        assert!(config.auth.jwt_secret_configured);
    }

    #[test]
    fn rejects_invalid_leader_election_intervals() {
        let repo = InMemoryConfigurationRepository::default()
            .with("LEADER_ELECTION_TTL_SECS", "2")
            .with("LEADER_ELECTION_RENEW_INTERVAL_MS", "3000");
        let service = ProductionConfigService::new(Box::new(repo));
        let error = service.load().unwrap_err();
        assert!(matches!(error, BootstrapConfigError::InvalidValue(_)));
    }
}
