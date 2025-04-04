use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Ingestion configuration
    pub ingestion: IngestionConfig,
    /// Database configuration
    pub database: DbConfig,
    /// Tracing configuration
    pub tracing: TracingConfig,
    /// AWS configuration
    pub aws: AwsConfig,
    /// Meta webhook configuration
    pub meta_webhook: MetaWebhookConfig,
    /// OpenID authentication configuration
    pub auth: AuthConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AuthConfig {
    /// OpenID issuer
    pub issuer_url: Url,
    /// OAuth client id
    pub client_id: String,
    /// OAuth client secret
    pub client_secret: String,
    /// OAuth redirect (callback) url
    pub redirect_url: Url,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct MetaWebhookConfig {
    /// The bearer token.
    pub token: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct AwsConfig {
    pub profile_name: Option<String>,
    pub endpoint_url: Option<Url>,
    #[serde(rename = "s3")]
    pub s3_config: AwsS3Config,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct AwsS3Config {
    pub bucket_name: String,
    pub public_url: Option<Url>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IngestionConfig {
    /// The API token for e-mail ingestion
    pub api_token: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TracingConfig {
    /// Enable tracing
    pub enabled: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DbConfig {
    /// Connection URL
    pub url: String,
    /// Maximum number of connections to keep in the connection pool
    #[serde(default = "default_max_db_connections")]
    pub max_connections: u32,
    /// Maximum idle duration for individual connections, in seconds
    #[serde(default = "default_db_idle_timeout", with = "humantime_serde")]
    pub idle_timeout: Duration,
}

pub const fn default_max_db_connections() -> u32 {
    crate::database::DEFAULT_MAX_CONNECTIONS
}

pub const fn default_db_idle_timeout() -> Duration {
    crate::database::DEFAULT_IDLE_TIMEOUT
}
