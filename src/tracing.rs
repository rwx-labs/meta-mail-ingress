use opentelemetry::KeyValue;
use opentelemetry_sdk::{
    trace::{BatchConfig, RandomIdGenerator},
    Resource,
};
use opentelemetry_semantic_conventions::{
    resource::{SERVICE_NAME, SERVICE_VERSION},
    SCHEMA_URL,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::cli::Format;
use crate::config;

// Create a Resource that captures information about the entity for which telemetry is recorded.
fn resource() -> Resource {
    Resource::from_schema_url(
        [
            KeyValue::new(SERVICE_NAME, env!("CARGO_PKG_NAME")),
            KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
        ],
        SCHEMA_URL,
    )
}

pub fn init(stdout_format: Format, tracing: &config::TracingConfig) -> miette::Result<()> {
    // Create a tracing layer with the configured tracer
    let telemetry_layer = if tracing.enabled {
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_trace_config(
                opentelemetry_sdk::trace::Config::default()
                    .with_id_generator(RandomIdGenerator::default())
                    .with_resource(resource()),
            )
            .with_batch_config(BatchConfig::default())
            .with_exporter(opentelemetry_otlp::new_exporter().tonic())
            .install_batch(opentelemetry_sdk::runtime::Tokio)
            .expect("could not create otlp pipeline");
        Some(tracing_opentelemetry::layer().with_tracer(tracer))
    } else {
        None
    };

    // initialize tracing
    let base = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "meta_mail_ingress=debug,tower_http=debug".into()),
        )
        .with(telemetry_layer);

    match stdout_format {
        Format::Json => base.with(tracing_subscriber::fmt::layer().json()).init(),
        Format::Pretty => base.with(tracing_subscriber::fmt::layer().pretty()).init(),
        Format::Compact => base.with(tracing_subscriber::fmt::layer().compact()).init(),
    };

    info!("tracing initialized");

    Ok(())
}
