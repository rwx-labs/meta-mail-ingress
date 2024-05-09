use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Ingestion configuration
    pub ingestion: IngestionConfig,
    /// Tracing configuration
    pub tracing: TracingConfig,
    /// AWS configuration
    pub aws: AwsConfig,
    /// Meta webhook configuration
    pub meta_webhook: MetaWebhookConfig,
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
