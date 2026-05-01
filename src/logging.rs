use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub fn init_tracing(debug: bool) -> WorkerGuard {
    let file_appender = tracing_appender::rolling::daily("./logs", "scraper.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let filter = if debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::try_from_default_env()
            .or_else(|_| EnvFilter::try_new("info"))
            .expect("valid filter")
    };

    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true)
        .compact();

    let file_layer = fmt::layer()
        .json()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_target(true)
        .with_file(true)
        .with_line_number(true);

    tracing_subscriber::registry()
        .with(filter)
        .with(stdout_layer)
        .with(file_layer)
        .init();

    guard
}
