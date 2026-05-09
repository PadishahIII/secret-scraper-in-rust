use anyhow::{Result, anyhow};
use async_trait::async_trait;
use derive_builder::Builder;
use std::{
    fs::{self, File},
    io::{self, stdout},
    path::PathBuf,
    sync::Arc,
};
use tokio::{runtime::Runtime, sync::Mutex};

use globwalk::GlobWalkerBuilder;

use crate::scraper::crawler;
use crate::{
    cli::Config,
    filter::{ChainedURLFilter, ChainedURLFilterBuilder},
    handler::RegexHandler,
    rate_limiter::{DomainRateLimiter, DomainRateLimiterBuilder},
    scanner::FileScanner,
    scraper::crawler::{Crawler, CrawlerBuilder},
    urlparser::{URLParser, URLParserBuilder},
};

#[async_trait]
pub trait ScanFacade {
    fn start(&mut self);
}
pub struct FileScannerFacade<'a> {
    scanner: FileScanner<PathBuf, RegexHandler>,
    outfile: Box<dyn io::Write + Send + 'a>,
}
impl<'a> FileScannerFacade<'a> {
    pub fn new(config: Config) -> Result<Self> {
        let out: Box<dyn io::Write + Send + 'a> = if let Some(f) = &config.outfile {
            Box::new(
                File::options()
                    .write(true)
                    .truncate(true)
                    .create(true)
                    .open(f)?,
            )
        } else {
            Box::new(stdout())
        };
        let handler = RegexHandler::new(config.custom_rules)?;
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
    fn start(&mut self) {
        Runtime::new()
            .map_err(|e| anyhow!("fail to create tokio runtime: {e}"))
            .unwrap()
            .block_on(async {
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
            })
    }
}

pub struct CrawlerFacade {
    system: actix::SystemRunner,
    crawler: Crawler<ChainedURLFilter, RegexHandler>,
}
impl CrawlerFacade {
    pub fn new(config: Config) -> Result<Self> {
        let system = actix::System::new();
        let crawler = system.block_on(async move {
            let mut seeds = vec![];
            if let Some(url) = config.url {
                seeds.push(url);
            }
            if let Some(url_file) = config.url_file {
                seeds.extend(
                    fs::read_to_string(url_file)?
                        .lines()
                        .map(str::trim)
                        .filter(|l| !l.is_empty())
                        .map(str::to_owned)
                        .collect::<Vec<String>>(),
                );
            }
            (!seeds.is_empty()).then_some(()).ok_or(anyhow!(
                "No target url found: at least one of '--url' and '--url-file' should be set"
            ))?;
            let mut filter_builder = ChainedURLFilter::builder();
            if let Some(allowed) = config.allow_domains {
                filter_builder.add_whitelist(allowed)?;
            }
            if let Some(blocked) = config.disallow_domains {
                filter_builder.add_blacklist(blocked)?;
            }

            let url_handler = RegexHandler::new(
                config
                    .url_find_rules
                    .into_iter()
                    .chain(config.js_find_rules)
                    .collect::<Vec<_>>(),
            )?;
            CrawlerBuilder::default()
                    .seeds(seeds)
                    .filter(filter_builder.build())
                    .parser(Arc::new(
                        URLParserBuilder::default().handler(url_handler).build()?,
                    ))
                    .rate_limiter(Arc::new(Mutex::new(
                        DomainRateLimiterBuilder::default()
                            .max_concurrency_per_domain(config.max_concurrency_per_domain)
                            .min_interval(config.min_request_interval)
                            .build()?,
                    )))
                    .secret_handler(Arc::new(RegexHandler::new(config.custom_rules)?))
                    .max_page_num(config.max_page)
                    .max_depth(config.max_depth)
                    .follow_redirects(config.follow_redirect)
                    .dangerous_paths(config.dangerous_paths)
                    .validate(config.validate)
                    .proxy(config.proxy)
                    .headers(config.custom_headers)
                    .timeout(config.timeout)
                    .build()
                    .map_err(|e| anyhow!("{}", e))
        })?;
        Ok(Self {
            system,
            crawler,
        })
    }
}
#[async_trait]
impl ScanFacade for CrawlerFacade {
    fn start(&mut self) {
        self.system.block_on(async {
            self.crawler
                .run()
                .await
                .map_err(|e| anyhow!("crawler error: {e}"))
                .unwrap();
        });
    }
}
