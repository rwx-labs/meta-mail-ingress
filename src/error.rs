use std::io;

use miette::Diagnostic;
use openidconnect::{DiscoveryError, HttpClientError as OidcHttpClientError};
use thiserror::Error;

use crate::auth::HttpClientError;

type OidcDiscoveryError = DiscoveryError<OidcHttpClientError<HttpClientError>>;

#[derive(Debug, Error, Diagnostic)]
pub enum Error {
    #[error("io error")]
    IoError(#[from] io::Error),
    #[error("could not create temporary file")]
    CreateTempFile(#[source] io::Error),
    #[error("post-processing failed: {0}")]
    PostProcessFailed(String),
    #[error("aws sdk error")]
    AwsS3Error(#[source] Box<aws_sdk_s3::Error>),
    #[error("s3 error")]
    S3PutObjectFailed(#[source] Box<aws_sdk_s3::Error>),
    #[error("could not read from bytestream")]
    ByteStream(#[source] Box<aws_sdk_s3::primitives::ByteStreamError>),
    #[error("reqwest error")]
    Reqwest(#[from] reqwest::Error),
    #[error("the tool `{0}' failed healthcheck, is it installed?")]
    ToolCheckFailed(String),
    #[error("could not install global tracing subscriber")]
    TracingTryInit(#[from] tracing_subscriber::util::TryInitError),
    #[error("could not build opentelemetry span exporter")]
    BuildOtelExporter(#[source] Box<dyn std::error::Error + Sync + Send>),
    #[error("could not connect to database")]
    OpenDatabase(#[source] sqlx::Error),
    #[error("database migration failed")]
    DatabaseMigration(#[from] sqlx::migrate::MigrateError),
    #[error("could not acquire handle from database connection pool")]
    DatabasePoolConnection(#[source] sqlx::Error),
    /// An error occurred while trying to retrieve provider metadata.
    #[error("Could not retrieve OpenID Connect provider metadata")]
    OidcDiscovery(#[from] OidcDiscoveryError),
    #[error("Internal error")]
    InternalError,
}
