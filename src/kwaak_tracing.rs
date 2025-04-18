use crate::repository::Repository;
use anyhow::Result;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{fmt, EnvFilter};

#[cfg(feature = "otel")]
pub struct Guard {
    otel: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
}

#[cfg(not(feature = "otel"))]
pub struct Guard {
    #[allow(dead_code)]
    otel: Option<()>,
}

#[cfg(feature = "otel")]
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

    if repository.config().otel_enabled {
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
    if repository.config().otel_enabled {
        println!("OpenTelemetry tracing enabled");
        if cfg!(feature = "otel") {
            let guard = init_otel(&mut layers)?;
            provider_for_guard = Some(guard);
        } else {
            eprintln!("OpenTelemetry tracing is enabled but the `otel` feature is not enabled");
        }
    }

    let registry = tracing_subscriber::registry()
        .with(env_filter_layer)
        .with(layers);
    registry.try_init()?;

    Ok(Guard {
        otel: provider_for_guard,
    })
}

#[cfg(not(feature = "otel"))]
fn init_otel<S>(
    _layers: &mut Vec<Box<dyn tracing_subscriber::Layer<S> + Send + Sync>>,
) -> Result<()>
where
    S: tracing::Subscriber + for<'span> LookupSpan<'span> + Send + Sync,
{
    Ok(())
}

#[cfg(feature = "otel")]
fn init_otel<S>(
    layers: &mut Vec<Box<dyn tracing_subscriber::Layer<S> + Send + Sync>>,
) -> Result<opentelemetry_sdk::trace::SdkTracerProvider>
where
    S: tracing::Subscriber + for<'span> LookupSpan<'span> + Send + Sync,
{
    use std::collections::HashMap;

    use anyhow::Context as _;
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_otlp::WithExportConfig as _;
    use opentelemetry_sdk::trace::SdkTracerProvider;

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_protocol(opentelemetry_otlp::Protocol::HttpBinary)
        .build()?;

    let service_name = if let Ok(service_name) = std::env::var("OTEL_SERVICE_NAME") {
        service_name
    } else {
        let resource_attributes = std::env::var("OTEL_RESOURCE_ATTRIBUTES")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.split_once('=')
                    .context("invalid OTEL_RESOURCE_ATTRIBUTES")
            })
            .map(|val| val.map(|(key, value)| (key.to_string(), value.to_string())))
            .collect::<Result<HashMap<String, String>>>()?;
        if let Some(service_name) = resource_attributes.get("service.name") {
            service_name.to_string()
        } else {
            "kwaak".to_string()
        }
    };

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(
            opentelemetry_sdk::Resource::builder()
                .with_service_name(service_name)
                .build(),
        )
        .build();

    let tracer = provider.tracer("kwaak");
    opentelemetry::global::set_tracer_provider(provider.clone());

    // Create a tracing layer with the configured tracer
    let layer = tracing_opentelemetry::OpenTelemetryLayer::new(tracer);
    layers.push(layer.boxed());

    Ok(provider)
}
