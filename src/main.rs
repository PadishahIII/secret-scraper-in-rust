mod cli;
mod facade;
mod filter;
mod handler;
mod logging;
mod output;
mod rate_limiter;
mod scanner;
mod scraper;
mod urlparser;

use std::{fs::File, io::BufWriter, path::PathBuf};

use crate::{
    cli::{CliConfigLayer, Config, FileConfigLayer},
    facade::{CrawlerFacade, FileScannerFacade, ScanFacade},
    logging::init_tracing,
    scraper::crawler::Crawler,
};
use clap::Parser;
use cli::LoadFromYaml;

fn main() {
    let cli_layer = CliConfigLayer::parse();

    let yaml_path = cli_layer.config.clone().or_else(|| {
        let default = PathBuf::from("settings.yaml");
        default.exists().then_some(default)
    });

    let yaml_layer = match yaml_path {
        Some(ref path) if path.exists() => Some(
            FileConfigLayer::load_from_yaml(path.clone()).unwrap_or_else(|e| {
                tracing::error!("Failed to load config file {}: {e}", path.display());
                std::process::exit(1);
            }),
        ),
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
            serde_yaml::to_writer(BufWriter::new(f), &Config::default_with_rules()).unwrap();
            println!("Default configuration written to settings.yaml");
            None
        }
    };

    let mut config = Config::default();
    if let Some(layer) = yaml_layer {
        config.apply_file_layer(layer).unwrap();
    }
    config.apply_cli_layer(cli_layer);

    if let Err(e) = config.validate() {
        tracing::error!("{e}");
        std::process::exit(1);
    }

    let _guard = init_tracing(config.debug);
    let mut facade: Box<dyn ScanFacade> = if config.local.is_some() {
        Box::new(
            FileScannerFacade::new(config)
                .map_err(|e| format!("fail to create FileScannerFacade: {e}"))
                .unwrap(),
        )
    } else {
        Box::new(
            CrawlerFacade::new(config)
                .map_err(|e| format!("fail to create CrawlerFacade: {e}"))
                .unwrap(),
        )
    };
    facade.start();
}
