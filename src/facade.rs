use anyhow::Result;
use async_trait::async_trait;
use serde::Serialize;
use std::{
    error::Error,
    fs::File,
    io::{self, stdout},
    path::PathBuf,
};

use globwalk::GlobWalkerBuilder;
use tokio::fs;

use crate::{cli::Config, handler::RegexHandler, scanner::FileScanner};

#[async_trait]
pub trait ScanFacade {
    async fn start(&mut self);
}
pub struct FileScannerFacade<'a> {
    scanner: FileScanner<PathBuf, RegexHandler>,
    outfile: Box<dyn io::Write + Send + 'a>,
}
impl<'a> FileScannerFacade<'a> {
    pub fn new(config: Config) -> Result<Self> {
        let out: Box<dyn io::Write + Send + 'a> = if let Some(f) = &config.outfile {
            Box::new(File::open(f)?)
        } else {
            Box::new(stdout())
        };
        let handler = RegexHandler::new(config.custom_rules);
        let base = config
            .local
            .as_ref()
            .ok_or(io::Error::other("'local' (base dir) not set"))?;
        let targets = if base.is_file() {
            vec![base.clone()]
        } else {
            GlobWalkerBuilder::new(base, "**/*")
                .build()?
                .filter_map(Result::ok)
                .filter(|f| f.path().is_file())
                .map(|f| f.path().to_path_buf())
                .collect()
        };
        let scanner = FileScanner::new(targets, handler);
        Ok(Self {
            scanner,
            outfile: out,
        })
    }
}
#[async_trait]
impl<'a> ScanFacade for FileScannerFacade<'a> {
    async fn start(&mut self) {
        match self.scanner.scan().await {
            Ok(res) => {
                self.outfile
                    .write_all(
                        serde_yaml::to_string(&res)
                            .map_err(|e| format!("fail to serialize scanner result: {e}"))
                            .unwrap()
                            .as_bytes(),
                    )
                    .map_err(|e| format!("fail to write scanner result to file: {e}"))
                    .unwrap();
            }
            Err(e) => {
                tracing::error!("scanner failed: {e}")
            }
        };
    }
}

pub struct CrawlerFacade {}
impl CrawlerFacade {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }
}
#[async_trait]
impl ScanFacade for CrawlerFacade {
    async fn start(&mut self) {
        todo!("implement me");
    }
}
