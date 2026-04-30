use std::process;

use clap::{Parser, Subcommand, ValueEnum};
use cli::Cli;
use slog::{error, log};
mod cli;

fn main() {
    let drain = slog::Discard; // todo
    let root = slog::Logger::root(drain, slog::o!());
    let cli = Cli::parse();
    if let Some(config_file) = cli.config {
        if !config_file.exists() {
            error!(root, "Config file {} does not exist", config_file.display());
            std::process::exit(1);
        }
        // todo: load config file
    }
}
