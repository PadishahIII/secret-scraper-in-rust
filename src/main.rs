mod cli;
mod facade;
mod logging;

use std::{fs::File, io::BufWriter, path::PathBuf};

use crate::{cli::Config, logging::init_tracing};
use clap::Parser;

fn main() {
    let mut config = Config::parse();
    if let Some(config_file) = config.config {
        if !config_file.exists() {
            tracing::error!("Config file {} does not exist", config_file.display());
            std::process::exit(1);
        }
        // ignore cli options
        config = Config::load_from_yaml(config_file).unwrap();
    } else {
        let default_cfg = Config::default();
        let mut p = PathBuf::new();
        p.push("settings.yaml");
        if !p.exists() {
            let f = File::options()
                .create(true)
                .write(true)
                .truncate(true)
                .open(p)
                .unwrap();
            serde_yaml::to_writer(BufWriter::new(f), &default_cfg).unwrap();
            println!("Default configuration written to settings.yaml");
        }
    }
    init_tracing(config.debug);
    // todo: init facade and run
}
