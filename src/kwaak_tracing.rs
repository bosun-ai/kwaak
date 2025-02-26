use crate::repository::Repository;
use anyhow::Result;
use tracing::level_filters::LevelFilter;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

pub struct Guard {
    otel: Option<TracerProvider>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        tracing::debug!("shutting down tracing");
        if let Some(provider) = self.otel.take() {
            if let Err(e) = provider.shutdown() {
                eprintln!("Failed to shutdown OpenTelemetry: {e:?}");
            }
        }
    }
}
/// Configures tracing for the app
///
/// # Panics
///
/// Panics if setting up tracing fails
pub fn init(repository: &Repository, tui_logger_enabled: bool) -> Result<Guard> {
    let log_dir = repository.config().log_dir();

    let file_appender = tracing_appender::rolling::daily(
        log_dir,
        format!("{}.log", repository.config().project_name),
    );

    let fmt_layer = fmt::layer().compact().with_writer(file_appender);

    // Logs the file layer will capture
    let mut env_filter_layer = EnvFilter::builder()
        .with_default_directive(LevelFilter::ERROR.into())
        .from_env_lossy();

    if cfg!(feature = "otel") && repository.config().otel_enabled {
        env_filter_layer = env_filter_layer
            .add_directive("swiftide=debug".parse().unwrap())
            .add_directive("swiftide_docker_executor=debug".parse().unwrap())
            .add_directive("swiftide_indexing=debug".parse().unwrap())
            .add_directive("swiftide_integrations=debug".parse().unwrap())
            .add_directive("swiftide_query=debug".parse().unwrap())
            .add_directive("swiftide_agents=debug".parse().unwrap())
            .add_directive("swiftide_core=debug".parse().unwrap())
            .add_directive("kwaak=debug".parse().unwrap());
    }

    // The log level tui logger will capture
    let default_level = if cfg!(debug_assertions) {
        log::LevelFilter::Info
    } else {
        log::LevelFilter::Warn
    };

    let mut layers = vec![fmt_layer.boxed()];

    if tui_logger_enabled {
        let tui_layer = tui_logger::tracing_subscriber_layer();
        tui_logger::init_logger(default_level)?;
        layers.push(tui_layer.boxed());
    }

    let mut provider_for_guard = None;
    if cfg!(feature = "otel") && repository.config().otel_enabled {
        println!("OpenTelemetry tracing enabled");
        let provider = init_otel();
        let tracer = provider.tracer("kwaak");
        opentelemetry::global::set_tracer_provider(provider.clone());
        provider_for_guard = Some(provider);

        // Create a tracing layer with the configured tracer
        let layer = OpenTelemetryLayer::new(tracer);

        layers.push(layer.boxed());
    }

    let registry = tracing_subscriber::registry()
        .with(env_filter_layer)
        .with(layers);
    registry.try_init()?;

    Ok(Guard {
        otel: provider_for_guard,
    })
}

#[cfg(feature = "otel")]
use opentelemetry::trace::TracerProvider as _;
#[cfg(feature = "otel")]
use opentelemetry_sdk::trace::TracerProvider;

#[cfg(feature = "otel")]
fn init_otel() -> TracerProvider {
    use std::collections::HashMap;

    use opentelemetry_sdk::runtime;
    use opentelemetry_sdk::trace::TracerProvider;

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .build()
        .expect("failed to create otlp exporter");

    let service_name = if let Ok(service_name) = std::env::var("OTEL_SERVICE_NAME") {
        service_name
    } else {
        let resource_attributes = std::env::var("OTEL_RESOURCE_ATTRIBUTES")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.split_once('=').expect("invalid OTEL_RESOURCE_ATTRIBUTES"))
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect::<HashMap<String, String>>();
        if let Some(service_name) = resource_attributes.get("service.name") {
            service_name.to_string()
        } else {
            "kwaak".to_string()
        }
    };

    TracerProvider::builder()
        .with_batch_exporter(exporter, runtime::Tokio)
        .with_resource(opentelemetry_sdk::Resource::new(vec![
            opentelemetry::KeyValue::new("service.name", service_name),
        ]))
        .build()
}
