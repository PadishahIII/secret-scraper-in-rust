mod cli;
mod logging;

use crate::{cli::Config, logging::init_tracing};
use clap::Parser;
use slog::error;

fn main() {
    let drain = slog::Discard; // todo
    let root = slog::Logger::root(drain, slog::o!());
    let cli = Config::parse();
    if let Some(config_file) = cli.config
        && !config_file.exists()
    {
        error!(root, "Config file {} does not exist", config_file.display());
        std::process::exit(1);
    }
    // todo: load config file
    init_tracing(cli.debug);
}
