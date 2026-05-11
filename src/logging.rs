//! Logging and tracing setup.

use std::{
    io::{self, Write},
    sync::atomic::{AtomicBool, Ordering},
};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, filter, fmt, prelude::*};

static LOGGING_DISABLED: AtomicBool = AtomicBool::new(false);

/// Log level selected by the CLI startup path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogLevel {
    /// Emit warnings and errors only.
    Warn,
    /// Emit informational logs plus warnings and errors.
    Info,
}

impl LogLevel {
    fn as_filter(self) -> &'static str {
        match self {
            Self::Warn => "warn",
            Self::Info => "info",
        }
    }
}

/// Select the CLI log level: `--verbose` enables info output, otherwise warning.
pub fn cli_log_level(verbose: bool) -> LogLevel {
    if verbose {
        LogLevel::Info
    } else {
        LogLevel::Warn
    }
}

/// Initialize stdout and file tracing with an explicit level.
///
/// Returns a [`WorkerGuard`] that must be kept alive for non-blocking file logs
/// to flush correctly.
pub fn init_tracing_with_level(level: LogLevel) -> WorkerGuard {
    LOGGING_DISABLED.store(false, Ordering::SeqCst);

    let file_appender = tracing_appender::rolling::daily("./logs", "scraper.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_new(level.as_filter()).expect("valid filter");
    let shutdown_filter = filter::filter_fn(|_| !LOGGING_DISABLED.load(Ordering::SeqCst));

    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true)
        .compact()
        .with_filter(shutdown_filter.clone());

    let file_layer = fmt::layer()
        .json()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_filter(shutdown_filter);

    tracing_subscriber::registry()
        .with(filter)
        .with(stdout_layer)
        .with(file_layer)
        .init();

    guard
}

/// Disable tracing output and print the shutdown notification.
pub fn notify_shutdown(mut writer: impl Write) -> io::Result<()> {
    disable_logging();
    writeln!(writer, "Shutdown...")
}

/// Disable future tracing output.
pub fn disable_logging() {
    LOGGING_DISABLED.store(true, Ordering::SeqCst);
}

/// Return whether shutdown has disabled logging.
#[cfg(test)]
pub fn logging_disabled() -> bool {
    LOGGING_DISABLED.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_notification_disables_logging_and_prints_message() {
        let mut out = Vec::new();

        notify_shutdown(&mut out).expect("notify shutdown");

        assert!(logging_disabled());
        assert_eq!(
            String::from_utf8(out).expect("utf8 output"),
            "Shutdown...\n"
        );
    }

    #[test]
    fn cli_log_level_defaults_to_warn_and_verbose_enables_info() {
        assert_eq!(cli_log_level(false), LogLevel::Warn);
        assert_eq!(cli_log_level(true), LogLevel::Info);
    }
}
