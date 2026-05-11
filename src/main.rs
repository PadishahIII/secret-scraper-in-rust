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

use std::{fs, fs::File, io::BufWriter, path::Path};

use crate::{
    cli::{CliConfigLayer, Config, FileConfigLayer},
    error::{Result, SecretScraperError},
    facade::{CrawlerFacade, FileScannerFacade, ScanFacade},
    logging::{cli_log_level, init_tracing_with_level},
};
use clap::Parser;
use cli::LoadFromYaml;
use owo_colors::OwoColorize;

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli_layer = CliConfigLayer::parse();

    let yaml_path = cli_layer.config.clone();

    let yaml_layer = if yaml_path.exists() {
        Some(
            FileConfigLayer::load_from_yaml(yaml_path.clone()).map_err(|e| {
                SecretScraperError::Other(format!(
                    "configuration error: failed to load config file {}: {e}",
                    yaml_path.display()
                ))
            })?,
        )
    } else {
        let f = File::options()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&yaml_path)
            .map_err(|e| {
                SecretScraperError::Io(std::io::Error::new(
                    e.kind(),
                    format!(
                        "failed to create default config file {}: {e}",
                        yaml_path.display()
                    ),
                ))
            })?;
        serde_yaml::to_writer(BufWriter::new(f), &Config::default_with_rules()).map_err(|e| {
            SecretScraperError::Other(format!(
                "failed to write default config file {}: {e}",
                yaml_path.display()
            ))
        })?;
        println!("Default configuration written to {}", yaml_path.display());
        None
    };

    let mut config = Config::default();
    if let Some(layer) = yaml_layer {
        config
            .apply_file_layer(layer)
            .map_err(|e| SecretScraperError::Other(format!("configuration error: {e}")))?;
    }
    config.apply_cli_layer(cli_layer);

    if let Err(e) = config.validate() {
        return Err(SecretScraperError::Other(format!(
            "configuration error: {e}"
        )));
    }

    print_startup_banner(&config)?;

    let _guard = init_tracing_with_level(cli_log_level(config.verbose));
    let is_local_scan = config.local.is_some();
    let facade: Box<dyn ScanFacade> = if is_local_scan {
        Box::new(FileScannerFacade::new(config).map_err(|e| {
            SecretScraperError::Other(format!("scan setup error: failed to create scanner: {e}"))
        })?)
    } else {
        Box::new(CrawlerFacade::new(config).map_err(|e| {
            SecretScraperError::Other(format!("scan setup error: failed to create crawler: {e}"))
        })?)
    };
    print_scan_start_status(is_local_scan);
    facade.scan()?;
    Ok(())
}

fn print_scan_start_status(is_local_scan: bool) {
    if is_local_scan {
        println!("Start to scan local files...");
    } else {
        println!("Start to crawl...");
    }
}

fn print_startup_banner(config: &Config) -> Result<()> {
    let banner = if let Some(local) = &config.local {
        format!(
            "Target files num: {}\nMax depth: N/A, Max page num: {}\nOutput file: {}",
            count_local_files(local)?,
            max_page_label(config),
            output_file_label(config)
        )
    } else {
        format!(
            "Target urls num: {}\nMax depth: {}, Max page num: {}\nOutput file: {}",
            count_target_urls(config)?,
            effective_max_depth(config),
            max_page_label(config),
            output_file_label(config)
        )
    };
    println!("{}", banner.bright_black());
    Ok(())
}

fn count_target_urls(config: &Config) -> Result<usize> {
    let direct_url_count = usize::from(
        config
            .url
            .as_ref()
            .is_some_and(|url| !url.trim().is_empty()),
    );
    let file_url_count = if let Some(path) = &config.url_file {
        fs::read_to_string(path)?
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .count()
    } else {
        0
    };
    Ok(direct_url_count + file_url_count)
}

fn count_local_files(path: &Path) -> Result<usize> {
    if path.is_file() {
        return Ok(1);
    }
    Ok(globwalk::GlobWalkerBuilder::new(path, "**/*")
        .build()
        .map_err(|e| {
            SecretScraperError::Other(format!(
                "scan setup error: failed to count local files in {}: {e}",
                path.display()
            ))
        })?
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.path().is_file())
        .count())
}

fn effective_max_depth(config: &Config) -> u32 {
    config.max_depth.unwrap_or(match config.mode {
        crate::cli::Mode::Normal => 1,
        crate::cli::Mode::Thorough => 2,
    })
}

fn max_page_label(config: &Config) -> String {
    config
        .max_page
        .map(|max_page| max_page.to_string())
        .unwrap_or_else(|| "unlimited".to_string())
}

fn output_file_label(config: &Config) -> String {
    config
        .outfile
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "stdout".to_string())
}
