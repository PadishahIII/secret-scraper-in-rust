mod cli;
mod error;
mod facade;
mod filter;
mod handler;
mod logging;
mod output;
mod rate_limiter;
mod scanner;
mod scraper;
mod urlparser;

use std::{fs::File, io::BufWriter};

use crate::{
    cli::{CliConfigLayer, Config, FileConfigLayer},
    facade::{CrawlerFacade, FileScannerFacade, ScanFacade},
    logging::init_tracing,
};
use clap::Parser;
use cli::LoadFromYaml;

fn main() {
    let cli_layer = CliConfigLayer::parse();

    let yaml_path = cli_layer.config.clone();

    let yaml_layer = if yaml_path.exists() {
        Some(
            FileConfigLayer::load_from_yaml(yaml_path.clone()).unwrap_or_else(|e| {
                eprintln!("Failed to load config file {}: {e}", yaml_path.display());
                std::process::exit(1);
            }),
        )
    } else {
        let f = File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&yaml_path)
            .unwrap();
        serde_yaml::to_writer(BufWriter::new(f), &Config::default_with_rules()).unwrap();
        println!("Default configuration written to {}", yaml_path.display());
        None
    };

    let mut config = Config::default();
    if let Some(layer) = yaml_layer {
        config.apply_file_layer(layer).unwrap();
    }
    config.apply_cli_layer(cli_layer);

    if let Err(e) = config.validate() {
        eprintln!("configuration error: {e}");
        std::process::exit(1);
    }

    let _guard = init_tracing(config.debug);
    let facade: Box<dyn ScanFacade> = if config.local.is_some() {
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
    facade.scan().unwrap();
}
