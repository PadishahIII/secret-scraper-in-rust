use std::{
    error::Error,
    fs::File,
    io::{self, stdout},
    path::PathBuf,
};

use globwalk::GlobWalkerBuilder;

use crate::{cli::Config, handler::RegexHandler, scanner::FileScanner};

pub struct FileScannerFacade<'a> {
    config: &'a Config,
    scanner: FileScanner<PathBuf, RegexHandler<'a>>,
    outfile: Box<dyn io::Write>,
}
impl<'a> FileScannerFacade<'a> {
    pub fn new(config: &'a Config) -> Result<Self, Box<dyn Error>> {
        let out: Box<dyn io::Write> = if let Some(f) = &config.outfile {
            Box::new(File::open(f)?)
        } else {
            Box::new(stdout())
        };
        let handler = RegexHandler::new(&config.custom_rules);
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
            config,
            scanner,
            outfile: out,
        })
    }
}

pub struct CrawlerFacade {}
