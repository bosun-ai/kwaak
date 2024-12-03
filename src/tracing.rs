use crate::repository::Repository;
use anyhow::Result;
use tracing::level_filters::LevelFilter;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{fmt, EnvFilter};

// For now just log to stdout and file
pub fn init(repository: &Repository) -> Result<()> {
    let log_dir = repository.config().log_dir();

    let file_appender = tracing_appender::rolling::daily(
        log_dir,
        format!("{}.log", repository.config().project_name),
    );

    let fmt_layer = fmt::layer().with_writer(file_appender);

    // Logs the file layer will capture
    let env_filter_layer = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy()
        .add_directive("h2=error".parse().unwrap())
        .add_directive("tower=error".parse().unwrap())
        .add_directive("tui_markdown=error".parse().unwrap());

    // The log level tui logger will capture
    let default_level = if cfg!(debug_assertions) {
        log::LevelFilter::Info
    } else {
        log::LevelFilter::Warn
    };
    let tui_layer = tui_logger::tracing_subscriber_layer();
    tui_logger::init_logger(default_level)?;

    let mut layers = vec![
        // env_filter_layer.boxed(),
        tui_layer.boxed(),
        fmt_layer.boxed(),
    ];

    if cfg!(feature = "otel") {
        dbg!("OpenTelemetry tracing enabled");
        let provider = otel_provider();
        let tracer = provider.tracer("kwaak");
        opentelemetry::global::set_tracer_provider(provider);

        // Create a tracing layer with the configured tracer
        let layer = OpenTelemetryLayer::new(tracer);

        layers.push(layer.boxed());
    }

    let registry = tracing_subscriber::registry()
        .with(env_filter_layer)
        .with(layers);
    registry.try_init()?;

    Ok(())
}

#[cfg(feature = "otel")]
use opentelemetry::trace::TracerProvider as _;
#[cfg(feature = "otel")]
use opentelemetry_sdk::trace::TracerProvider;

#[cfg(feature = "otel")]
fn otel_provider() -> TracerProvider {
    use opentelemetry_sdk::runtime;
    use opentelemetry_sdk::trace::TracerProvider;

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .build()
        .expect("failed to create otlp exporter");

    TracerProvider::builder()
        .with_batch_exporter(exporter, runtime::Tokio)
        .with_config(opentelemetry_sdk::trace::Config::default().with_resource(
            opentelemetry_sdk::Resource::new(vec![opentelemetry::KeyValue::new(
                "service.name",
                "kwaak",
            )]),
        ))
        .build()
}
