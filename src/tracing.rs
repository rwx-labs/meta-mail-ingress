use std::env;

use opentelemetry::trace::TracerProvider;
use opentelemetry::InstrumentationScope;
use opentelemetry_resource_detectors::{
    HostResourceDetector, K8sResourceDetector, OsResourceDetector,
};
use opentelemetry_sdk::resource::ResourceDetector;
use opentelemetry_sdk::Resource;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::cli::Format;
use crate::config;
use crate::Error;

/// Returns a list of resource detectors to use to enrich OTel attributes.
fn otel_resource_detectors() -> Vec<Box<dyn ResourceDetector>> {
    vec![
        Box::new(OsResourceDetector),
        Box::new(HostResourceDetector::default()),
        Box::new(K8sResourceDetector),
    ]
}

pub fn try_init(stdout_format: &Format, tracing: &config::TracingConfig) -> Result<(), Error> {
    // Create a tracing layer with the configured tracer
    let telemetry_layer = if tracing.enabled {
        // Set up the OTLP exporter
        let otlp_exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .build()
            .map_err(|e| Error::BuildOtelExporter(Box::new(e)))?;
        // Set up resource detectors to enrich otel attributes
        let res_detectors = otel_resource_detectors();
        // Resource detectors for tracing context
        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_batch_exporter(otlp_exporter)
            .with_resource(
                Resource::builder()
                    .with_service_name(env!("CARGO_PKG_NAME"))
                    .with_detectors(&res_detectors)
                    .build(),
            )
            .build();
        let scope = InstrumentationScope::builder(env!("CARGO_PKG_NAME"))
            .with_version(env!("CARGO_PKG_VERSION"))
            .with_schema_url("https://opentelemetry.io/schema/1.0.0")
            .build();
        let tracer = provider.tracer_with_scope(scope);
        let layer = tracing_opentelemetry::layer().with_tracer(tracer);

        Some(layer)
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
        Format::Json => base
            .with(tracing_subscriber::fmt::layer().json())
            .try_init()?,
        Format::Pretty => base
            .with(tracing_subscriber::fmt::layer().pretty())
            .try_init()?,
        Format::Compact => base
            .with(tracing_subscriber::fmt::layer().compact())
            .try_init()?,
    }

    info!("tracing initialized");

    Ok(())
}
