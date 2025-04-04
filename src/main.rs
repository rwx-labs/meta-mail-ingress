use std::sync::Arc;

use ::tracing::{debug, info};
use auth::Authenticator;
use aws_config::BehaviorVersion;
use aws_sdk_s3 as aws_s3;
use figment::{
    Figment,
    providers::{Env, Format, Toml},
};
use miette::IntoDiagnostic;
use tokio::sync::Mutex;
use tower_sessions_sqlx_store::PostgresStore;

mod api;
mod auth;
mod cli;
mod config;
mod database;
mod error;
mod handler;
mod http;
mod postprocess;
mod tracing;

pub use config::Config;
pub use database::Database;
pub use error::Error;
pub use handler::MailHandler;

#[derive(Debug, Clone)]
pub struct AppState {
    pub api_token: String,
    pub mail_handler: Arc<Mutex<MailHandler>>,
    pub authenticator: Authenticator,
    pub session_store: PostgresStore,
    pub database: Database,
}

async fn load_aws_config(app_aws_config: &config::AwsConfig) -> aws_config::SdkConfig {
    let mut config_loader = aws_config::defaults(BehaviorVersion::latest());

    // Override the profile name to load.
    if let Some(ref profile_name) = app_aws_config.profile_name {
        config_loader = config_loader.profile_name(profile_name);
    }

    // Override the endpoint URL for all AWS services if provided.
    if let Some(ref endpoint_url) = app_aws_config.endpoint_url {
        config_loader = config_loader.endpoint_url(endpoint_url.as_str());
    }

    config_loader.load().await
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    let opts: cli::Opts = argh::from_env();
    let config: Config = Figment::new()
        .merge(Toml::file(opts.config_path))
        .merge(Env::raw().lowercase(false).split("__"))
        .extract()
        .into_diagnostic()?;

    tracing::try_init(&opts.format, &config.tracing).expect("could not initialize tracing");

    let sdk_config = load_aws_config(&config.aws).await;
    let s3_client = aws_s3::Client::new(&sdk_config);
    let postprocessors = postprocess::init()?;
    let mail_handler = Arc::new(Mutex::new(MailHandler::new(
        s3_client,
        config.aws.s3_config.clone(),
        config.meta_webhook.token,
        postprocessors,
    )));

    debug!("connecting to database");
    let db = database::connect(config.database.url.as_str(), &config.database).await?;
    info!("connected to database");

    debug!("running database migrations");
    database::migrate(db.clone()).await?;
    debug!("database migrations complete");

    debug!("configuring authenticator");
    let authenticator = Authenticator::discover(
        db.clone(),
        config.auth.issuer_url.clone(),
        config.auth.client_id.clone(),
        config.auth.client_secret.clone(),
        config.auth.redirect_url.clone(),
    )
    .await?;
    debug!("finished configuration authenticator");

    // Set up the session layer
    debug!("creating session store");
    let session_store = PostgresStore::new(db.clone());
    debug!("migrating session store");
    session_store.migrate().await.into_diagnostic()?;

    let app_state = AppState {
        api_token: config.ingestion.api_token,
        mail_handler,
        authenticator,
        session_store,
        database: db.clone(),
    };

    http::start_server(app_state).await?;

    Ok(())
}
