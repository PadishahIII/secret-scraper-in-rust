//! Logging and tracing setup.

use std::{
    io::{self, Write},
    sync::atomic::{AtomicBool, Ordering},
};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, filter, fmt, prelude::*};

static LOGGING_DISABLED: AtomicBool = AtomicBool::new(false);

/// Initialize stdout and file tracing.
///
/// Returns a [`WorkerGuard`] that must be kept alive for non-blocking file logs
/// to flush correctly.
pub fn init_tracing(debug: bool) -> WorkerGuard {
    LOGGING_DISABLED.store(false, Ordering::SeqCst);

    let file_appender = tracing_appender::rolling::daily("./logs", "scraper.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let filter = if debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::try_from_default_env()
            .or_else(|_| EnvFilter::try_new("info"))
            .expect("valid filter")
    };
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
}
