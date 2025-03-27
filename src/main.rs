use std::sync::Arc;

use aws_config::BehaviorVersion;
use aws_sdk_s3 as aws_s3;
use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use miette::IntoDiagnostic;
use tokio::sync::Mutex;

mod api;
mod cli;
mod config;
mod error;
mod handler;
mod http;
mod postprocess;
mod tracing;

pub use config::Config;
pub use error::Error;
pub use handler::MailHandler;

#[derive(Debug, Clone)]
pub struct AppState {
    pub api_token: String,
    pub mail_handler: Arc<Mutex<MailHandler>>,
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

    tracing::init(&opts.format, &config.tracing);

    let sdk_config = load_aws_config(&config.aws).await;
    let s3_client = aws_s3::Client::new(&sdk_config);
    let postprocessors = postprocess::init()?;
    let mail_handler = Arc::new(Mutex::new(MailHandler::new(
        s3_client,
        config.aws.s3_config.clone(),
        config.meta_webhook.token,
        postprocessors,
    )));
    let app_state = AppState {
        api_token: config.ingestion.api_token,
        mail_handler,
    };

    http::start_server(app_state).await?;

    Ok(())
}
