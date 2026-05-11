use anyhow::{Result, anyhow};
use async_trait::async_trait;
use csv::Writer;
use derive_builder::Builder;
use reqwest::header::{self, HeaderValue};
use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, stdout},
    path::PathBuf,
    sync::Arc,
};
use tokio::{runtime::Runtime, sync::Mutex};
use tracing::info;
use url::Url;

use globwalk::GlobWalkerBuilder;

use crate::{
    cli::Config,
    filter::{ChainedURLFilter, ChainedURLFilterBuilder},
    handler::RegexHandler,
    output::{URLType, output_csv},
    rate_limiter::{DomainRateLimiter, DomainRateLimiterBuilder},
    scanner::FileScanner,
    scraper::crawler::{Crawler, CrawlerBuilder},
    urlparser::{URLNode, URLParser, URLParserBuilder},
};
use crate::{output::Formatter, scraper::crawler};

#[async_trait]
pub trait ScanFacade {
    fn start(self);
}
pub struct FileScannerFacade<'a> {
    scanner: FileScanner<PathBuf, RegexHandler>,
    outfile: Box<dyn io::Write + Send + 'a>,
}
impl<'a> FileScannerFacade<'a> {
    pub fn new(config: Config) -> Result<Self> {
        let base = config
            .local
            .as_ref()
            .ok_or(io::Error::other("'local' (base dir) not set"))?;
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
    fn start(mut self) {
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
    outfile: Option<Box<dyn io::Write>>,
    outfile_name: Option<String>,
    formatter: Formatter,
    // display options
    hide_regex: bool,
    show_detail: bool,
}
impl CrawlerFacade {
    pub fn new(config: Config) -> Result<Self> {
        let system = actix::System::new();
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

        let mut headers = config.custom_headers.unwrap_or_default();
        if let Some(ua) = config.user_agent
            && !ua.trim().is_empty()
        {
            headers.insert(
                header::USER_AGENT,
                HeaderValue::from_str(&ua).map_err(|e| anyhow!("fail to specify UA: {e}"))?,
            );
        }
        if let Some(cookie) = config.cookie {
            headers.insert(
                header::COOKIE,
                HeaderValue::from_str(&cookie).map_err(|e| anyhow!("fail to set cookie: {e}"))?,
            );
        }
        let mut max_depth;
        match config.mode {
            crate::cli::Mode::Normal => max_depth = Some(1),
            crate::cli::Mode::Thorough => max_depth = Some(2),
        }
        if let Some(m) = config.max_depth {
            max_depth = Some(m);
        }

        let outfile: Option<Box<dyn io::Write>> = if let Some(f) = &config.outfile {
            Some(Box::new(
                File::options()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(f)
                    .map_err(|e| anyhow!("fail to open outfile: {e}",))?,
            ))
        } else {
            None
        };
        let outfile_name = if let Some(f) = &config.outfile {
            f.to_str().and_then(|f| Some(f.to_string()))
        } else {
            None
        };

        let crawler = CrawlerBuilder::default()
            .seeds(seeds)
            .filter(filter_builder.build())
            .parser(Arc::new(
                URLParserBuilder::default().handler(url_handler).build()?,
            ))
            .rate_limiter(Arc::new(Mutex::new(
                DomainRateLimiterBuilder::default()
                    .max_concurrency_per_domain(config.max_concurrency_per_domain)
                    .min_interval(config.min_request_interval)
                    .build()
                    .map_err(|e| anyhow!("fail to build rate limiter"))?,
            )))
            .secret_handler(Arc::new(RegexHandler::new(config.custom_rules)?))
            .max_page_num(config.max_page)
            .max_depth(max_depth)
            .follow_redirects(config.follow_redirect)
            .dangerous_paths(config.dangerous_paths)
            .validate(config.validate)
            .proxy(config.proxy)
            .headers(Some(headers))
            .timeout(config.timeout)
            .build()
            .map_err(|e| anyhow!("fail to build crawler: {e}"))?;
        Ok(Self {
            system,
            crawler: crawler,
            outfile,
            outfile_name,
            formatter: Formatter::new(config.status_filter),
            hide_regex: config.hide_regex,
            show_detail: config.detail,
        })
    }
}
#[async_trait]
impl ScanFacade for CrawlerFacade {
    fn start(mut self) {
        self.system.block_on(async {
            self.crawler
                .run()
                .await
                .map_err(|e| anyhow!("crawler error: {e}"))
                .unwrap();
        });
        let res = self
            .crawler
            .result()
            .map_err(|e| anyhow!("fail to get crawler result: {e}"))
            .unwrap();
        if let Some(f) = self.outfile {
            let outfile_name = &self.outfile_name.unwrap();
            let c = output_csv(Box::new(f), &res.urls, &res.secrets)
                .map_err(|e| anyhow!("fail to write crawler result to file {}: {e}", outfile_name))
                .unwrap();
            info!("{} records written to {}", c, outfile_name);
        }
        let hosts = self.formatter.found_domains(res.hosts.iter().collect());
        if self.show_detail {
            tracing::info!(
                "URL Hierarcy:\n{}",
                self.formatter.format_url_hierarchy(&res.urls)
            );
            if !self.hide_regex {
                tracing::info!("Secrets:\n{}", self.formatter.format_secrets(&res.secrets));
            }
            tracing::info!("JS:\n{}", self.formatter.format_js(&res.js));
            tracing::info!(
                "Related Domains:\n{}",
                self.formatter.format_found_domains(hosts)
            );
        } else {
            tracing::info!(
                "URL:\n{}",
                self.formatter
                    .format_url_per_domain(&hosts, &res.urls, URLType::URL)
            );
            tracing::info!(
                "JS:\n{}",
                self.formatter
                    .format_url_per_domain(&hosts, &res.js, URLType::JS)
            );
            tracing::info!(
                "Related Domains:\n{}",
                self.formatter.format_found_domains(hosts)
            );
            if !self.hide_regex {
                tracing::info!("Secrets:\n{}", self.formatter.format_secrets(&res.secrets));
            }
        }
    }
}
