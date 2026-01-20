use crate::config::{LogFormat, LogLevel, LoggingConfig};
use tracing::Level;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize the logging system based on configuration
pub fn init_logger(config: &LoggingConfig) -> Result<(), Box<dyn std::error::Error>> {
    let level = match config.level {
        LogLevel::Trace => Level::TRACE,
        LogLevel::Debug => Level::DEBUG,
        LogLevel::Info => Level::INFO,
        LogLevel::Warn => Level::WARN,
        LogLevel::Error => Level::ERROR,
    };

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level.to_string()));

    let registry = tracing_subscriber::registry().with(env_filter);

    match (config.log_to_console, config.log_to_file) {
        (true, true) => {
            // Log to both console and file
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&config.log_file_path)?;

            let file_layer = match config.format {
                LogFormat::Json => fmt::layer()
                    .json()
                    .with_writer(std::sync::Mutex::new(file))
                    .boxed(),
                LogFormat::Pretty => fmt::layer()
                    .pretty()
                    .with_writer(std::sync::Mutex::new(file))
                    .boxed(),
            };

            let console_layer = match config.format {
                LogFormat::Json => fmt::layer().json().boxed(),
                LogFormat::Pretty => fmt::layer().pretty().boxed(),
            };

            registry.with(console_layer).with(file_layer).init();
        }
        (true, false) => {
            // Log to console only
            let console_layer = match config.format {
                LogFormat::Json => fmt::layer().json(),
                LogFormat::Pretty => fmt::layer().pretty(),
            };

            registry.with(console_layer).init();
        }
        (false, true) => {
            // Log to file only
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&config.log_file_path)?;

            let file_layer = match config.format {
                LogFormat::Json => fmt::layer()
                    .json()
                    .with_writer(std::sync::Mutex::new(file)),
                LogFormat::Pretty => fmt::layer()
                    .pretty()
                    .with_writer(std::sync::Mutex::new(file)),
            };

            registry.with(file_layer).init();
        }
        (false, false) => {
            // No logging (fallback to console with minimal output)
            registry.with(fmt::layer().compact()).init();
        }
    }

    tracing::info!("Logger initialized at {} level", config.level);

    Ok(())
}

/// Initialize a simple logger with default settings (for testing)
pub fn init_simple_logger() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_init_simple_logger() {
        // Should not panic
        init_simple_logger();
    }

    #[test]
    fn test_logger_config() {
        let config = LoggingConfig {
            level: LogLevel::Debug,
            format: LogFormat::Json,
            log_to_file: false,
            log_file_path: PathBuf::from("/tmp/test.log"),
            log_to_console: true,
            enable_metrics: false,
            metrics_addr: None,
        };

        // This would initialize the logger
        // We can't test it fully in unit tests as it can only be initialized once
        assert_eq!(config.level, LogLevel::Debug);
    }
}
