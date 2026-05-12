//! High-level scan facades for crawler and local file scanning workflows.

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use ignore::WalkBuilder;
use reqwest::header::{self, HeaderValue};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self},
    path::PathBuf,
    sync::Arc,
};
use tokio::{
    runtime::{self, Runtime},
    signal,
    sync::Mutex,
    task,
};
use tokio_util::{sync::CancellationToken, task::TaskTracker};

use crate::output::Formatter;
use crate::{
    cli::Config,
    error::{Result as SecretScraperResult, SecretScraperError},
    filter::ChainedURLFilter,
    handler::{RegexHandler, Secret},
    output::{URLType, output_csv},
    rate_limiter::DomainRateLimiterBuilder,
    scanner::FileScanner,
    scraper::crawler::{Crawler, CrawlerBuilder, CrawlerResult},
    urlparser::URLParserBuilder,
};

fn notify_ctrl_c_shutdown(shutdown: CancellationToken) {
    let _ = crate::logging::notify_shutdown(io::stdout());
    shutdown.cancel();
}

#[async_trait]
/// Common interface implemented by high-level scan facades.
pub trait ScanFacade {
    /// Run the scan and return a typed result.
    fn scan(self: Box<Self>) -> ScanStdResult;
}
/// Standard result type returned by scan facades.
pub type ScanStdResult = SecretScraperResult<ScanResult>;
/// Result payload produced by a scan facade.
#[allow(unused)]
pub enum ScanResult {
    /// Result of a local file scan keyed by scanned path.
    LocalScanResult(HashMap<PathBuf, HashSet<Secret>>),
    /// Result of a web crawl.
    CrawlResult(CrawlerResult),
}
/// Facade for recursively scanning local files with configured rules.
pub struct FileScannerFacade<'a> {
    scanner: FileScanner<PathBuf, RegexHandler>,
    formatter: Formatter,
    outfile: Option<Box<dyn io::Write + Send + 'a>>,
    shutdown: CancellationToken,
}
impl<'a> FileScannerFacade<'a> {
    /// Build a local file scanner facade from [`Config`].
    pub fn new(config: Config) -> Result<Self> {
        Self::with_shutdown(config, CancellationToken::new())
    }

    /// Build a local file scanner facade with a cooperative shutdown token.
    pub fn with_shutdown(config: Config, shutdown: CancellationToken) -> Result<Self> {
        let base = config
            .local
            .as_ref()
            .ok_or(io::Error::other("'local' (base dir) not set"))?;
        let out: Option<Box<dyn io::Write + Send + 'a>> = config
            .outfile
            .as_ref()
            .map(|f| -> io::Result<Box<dyn io::Write + Send + 'a>> {
                Ok(Box::new(
                    File::options()
                        .write(true)
                        .truncate(true)
                        .create(true)
                        .open(f)?,
                ))
            })
            .transpose()?;
        let handler = RegexHandler::new(config.custom_rules)?;
        let targets = if base.is_file() {
            vec![base.clone()]
        } else {
            WalkBuilder::new(base)
                .build()
                .filter_map(Result::ok)
                .filter(|f| f.path().is_file())
                .map(|f| f.path().to_path_buf())
                .collect()
        };
        let scanner = FileScanner::with_shutdown(targets, handler, shutdown.clone());
        Ok(Self {
            scanner,
            formatter: Formatter::new(config.status_filter),
            outfile: out,
            shutdown,
        })
    }
}
#[async_trait]
impl<'a> ScanFacade for FileScannerFacade<'a> {
    fn scan(self: Box<Self>) -> ScanStdResult {
        let shutdown = self.shutdown.clone();
        Runtime::new()
            .map_err(|e| {
                SecretScraperError::Runtime(format!("fail to create tokio runtime: {e:?}"))
            })?
            .block_on(async {
                task::spawn(async move {
                    if signal::ctrl_c().await.is_ok() {
                        notify_ctrl_c_shutdown(shutdown);
                    }
                });
                match self.scanner.scan().await {
                    Ok(res) => {
                        println!("Secrets: {}", self.formatter.format_local_secrets(&res));
                        if let Some(mut out) = self.outfile {
                            out.write_all(
                                serde_yaml::to_string(
                                    &res.iter()
                                        .filter(|(_, secrets)| !secrets.is_empty())
                                        .map(|(path, secrets)| (*path, secrets))
                                        .collect::<HashMap<&PathBuf, &HashSet<Secret>>>(),
                                )
                                .map_err(SecretScraperError::Yaml)?
                                .as_bytes(),
                            )
                            .map_err(SecretScraperError::Io)?;
                        }
                        Ok(ScanResult::LocalScanResult(
                            res.into_iter().map(|(k, v)| (k.clone(), v)).collect(),
                        ))
                    }
                    Err(e) => {
                        tracing::error!("local scanner failed: {e:?}");
                        Err(SecretScraperError::Scanner(format!("{:?}", e)))
                    }
                }
            })
    }
}

/// Facade for crawling web targets with configured rules.
pub struct CrawlerFacade {
    system: actix::SystemRunner,
    crawler: Crawler<ChainedURLFilter, RegexHandler>,
    outfile: Option<Box<dyn io::Write>>,
    outfile_name: Option<String>,
    formatter: Formatter,
    // display options
    hide_regex: bool,
    show_detail: bool,

    shutdown: CancellationToken,
    task_tracker: Arc<TaskTracker>,
}
impl CrawlerFacade {
    /// Build a crawler facade from [`Config`].
    pub fn new(config: Config) -> Result<Self> {
        Self::with_shutdown(config, CancellationToken::new())
    }

    /// Build a crawler facade with a cooperative shutdown token.
    pub fn with_shutdown(config: Config, shutdown: CancellationToken) -> Result<Self> {
        let system = actix::System::with_tokio_rt(|| {
            runtime::Builder::new_multi_thread()
                .worker_threads(num_cpus::get())
                .enable_all()
                .build()
                .unwrap()
        });
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
            f.to_str().map(|f| f.to_string())
        } else {
            None
        };

        let shutdown_clone = shutdown.clone();
        let task_tracker = Arc::new(TaskTracker::new());
        let tracker_clone = task_tracker.clone();

        let crawler = system.block_on(async move {
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
                        .build()
                        .map_err(|e| anyhow!("fail to build rate limiter: {e}"))?,
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
                .shutdown(shutdown_clone)
                .task_tracker(tracker_clone)
                .build()
                .map_err(|e| anyhow!("fail to build crawler: {e}"))
        })?;
        Ok(Self {
            system,
            crawler,
            outfile,
            outfile_name,
            shutdown,
            task_tracker,
            formatter: Formatter::new(config.status_filter),
            hide_regex: config.hide_regex,
            show_detail: config.detail,
        })
    }
}
#[async_trait]
impl ScanFacade for CrawlerFacade {
    fn scan(mut self: Box<Self>) -> ScanStdResult {
        let shutdown = self.shutdown.clone();
        let tracker = self.task_tracker.clone();
        self.system.block_on(async {
            task::spawn(async move {
                if signal::ctrl_c().await.is_ok() {
                    notify_ctrl_c_shutdown(shutdown);
                    tracker.close();
                    tracker.wait().await;
                }
            });
            self.crawler
                .run()
                .await
                .map_err(|e| SecretScraperError::Crawler(format!("{:?}", e)))
        })?;
        let res = self.crawler.result().map_err(|e| {
            SecretScraperError::Crawler(format!("fail to get crawler result: {e:?}"))
        })?;
        if let Some(f) = self.outfile {
            let outfile_name = self.outfile_name.as_deref().ok_or_else(|| {
                SecretScraperError::Output("outfile writer was set without an output path".into())
            })?;
            let c = output_csv(Box::new(f), &res.urls, &res.secrets).map_err(|e| {
                SecretScraperError::Output(format!(
                    "fail to write crawler result to file {}: {e}",
                    outfile_name
                ))
            })?;
            println!("{} records written to {}", c, outfile_name);
        }
        let hosts = self.formatter.found_domains(res.hosts.iter().collect());
        if self.show_detail {
            println!(
                "\nURL Hierarcy:\n{}",
                self.formatter.format_url_hierarchy(&res.urls)
            );
            if !self.hide_regex {
                println!(
                    "\nSecrets:\n{}",
                    self.formatter.format_secrets(&res.secrets)
                );
            }
            println!("\nJS:\n{}", self.formatter.format_js(&res.js));
            println!(
                "\nRelated Domains:\n{}",
                self.formatter.format_found_domains(hosts)
            );
        } else {
            println!(
                "\nURL:\n{}",
                self.formatter
                    .format_url_per_domain(&hosts, &res.urls, URLType::Url)
            );
            println!(
                "\nJS:\n{}",
                self.formatter
                    .format_url_per_domain(&hosts, &res.js, URLType::JS)
            );
            println!(
                "\nRelated Domains:\n{}",
                self.formatter.format_found_domains(hosts)
            );
            if !self.hide_regex {
                println!(
                    "\nSecrets:\n{}",
                    self.formatter.format_secrets(&res.secrets)
                );
            }
        }
        Ok(ScanResult::CrawlResult(res))
    }
}
