mod cli;
mod facade;
mod logging;

use std::{fs::File, io::BufWriter, path::PathBuf};

use crate::{
    cli::{Config, ConfigLayer},
    logging::init_tracing,
};
use clap::Parser;

fn main() {
    let cli_layer = ConfigLayer::parse();

    let yaml_path = cli_layer
        .config
        .clone()
        .or_else(|| {
            let default = PathBuf::from("settings.yaml");
            default.exists().then_some(default)
        });

    let yaml_layer = match yaml_path {
        Some(ref path) if path.exists() => {
            Some(ConfigLayer::load_from_yaml(path.clone()).unwrap_or_else(|e| {
                tracing::error!("Failed to load config file {}: {e}", path.display());
                std::process::exit(1);
            }))
        }
        Some(path) => {
            tracing::error!("Config file {} does not exist", path.display());
            std::process::exit(1);
        }
        None => {
            let default_path = PathBuf::from("settings.yaml");
            let f = File::options()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&default_path)
                .unwrap();
            serde_yaml::to_writer(BufWriter::new(f), &Config::default()).unwrap();
            println!("Default configuration written to settings.yaml");
            None
        }
    };

    let mut config = Config::default();
    if let Some(layer) = yaml_layer {
        config.apply(layer);
    }
    config.apply(cli_layer);

    if let Err(e) = config.validate() {
        tracing::error!("{e}");
        std::process::exit(1);
    }

    let _guard = init_tracing(config.debug);
    // todo: init facade and run
}
