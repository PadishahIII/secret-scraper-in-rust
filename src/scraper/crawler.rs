//! Crawl scheduler and result aggregation.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    thread::available_parallelism,
};

use actix::{Actor, Addr};
use anyhow::{Result, anyhow};
use derive_builder::Builder;
use reqwest::header::HeaderMap;
use reqwest::redirect::Policy;
use reqwest::{Client, Proxy};
use serde::Serialize;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::task;
use tracing::{debug, error, info, warn};
use url::Url;

use crate::scraper::{
    bo::{FetchMessage, FetchResult},
    worker::{Worker, WorkerBuilder},
};
use crate::urlparser::{URLNode, URLNodeBuilder, URLParser};
use crate::{filter::URLFilter, handler::Handler};
use crate::{handler::Secret, scraper::bo::ScrapeMessage};
use crate::{rate_limiter::DomainRateLimiter, scraper::bo::ScrapeResult};
use crate::{scraper::bo::ScrapeError, urlparser::ResponseStatus};
use derive_builder::UninitializedFieldError;
use std::time::Duration;

static MAX_INFLIGHT_TASKS: usize = 256;
static MAX_REDIRECT: usize = 5;

/// Error returned while building a [`Crawler`].
#[derive(Debug)]
#[allow(unused)]
pub enum CrawlerBuildError {
    /// A required builder field was not initialized.
    UninitializedFieldError(String),
    /// Any other builder error.
    Other(anyhow::Error),
}
impl From<UninitializedFieldError> for CrawlerBuildError {
    fn from(value: UninitializedFieldError) -> Self {
        Self::UninitializedFieldError(value.to_string())
    }
}
impl From<anyhow::Error> for CrawlerBuildError {
    fn from(value: anyhow::Error) -> Self {
        Self::Other(value)
    }
}

#[derive(Default)]
struct CrawlerState {
    in_flight: usize,
    visited_urls: HashSet<Url>,
    working_queue: VecDeque<Url>,
    // all urls found, regardless of visited or not
    url_bucket: HashMap<Url, URLNode>,
    urls: HashMap<Url, HashSet<Url>>, // index
    js: HashMap<Url, HashSet<Url>>,
    page_cnt: u32,
    url_secrets: HashMap<Url, HashSet<Secret>>,
}
/// Aggregated crawler output.
#[derive(Serialize)]
pub struct CrawlerResult {
    #[serde(rename = "found_hostnames")]
    /// Discovered host nodes.
    pub hosts: HashSet<URLNode>,
    #[serde(rename = "url_hierarchy")]
    /// Parent-child non-JavaScript URL relationships.
    pub urls: HashMap<URLNode, HashSet<URLNode>>,
    #[serde(rename = "js_hierarchy")]
    /// Parent-child JavaScript URL relationships.
    pub js: HashMap<URLNode, HashSet<URLNode>>,
    /// Secrets found per URL.
    pub secrets: HashMap<URLNode, HashSet<Secret>>,
}
impl AsRef<CrawlerResult> for CrawlerResult {
    fn as_ref(&self) -> &CrawlerResult {
        self
    }
}
impl TryFrom<CrawlerState> for CrawlerResult {
    type Error = anyhow::Error;
    fn try_from(value: CrawlerState) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            hosts: value
                .url_bucket
                .keys()
                .map(|url| {
                    URLNodeBuilder::default()
                        .url(url.to_string())
                        .build()
                        .map_err(|e| anyhow!("URLNodeBuilder error: {e}"))
                })
                .collect::<anyhow::Result<HashSet<URLNode>>>()?,
            urls: value
                .urls
                .into_iter()
                .map(|(url, set)| {
                    let node = value
                        .url_bucket
                        .get(&url)
                        .cloned()
                        .ok_or_else(|| anyhow!("fatal: no such entry: {url}"))?;
                    let children = set
                        .into_iter()
                        .map(|u| {
                            value
                                .url_bucket
                                .get(&u)
                                .cloned()
                                .ok_or_else(|| anyhow!("fatal: no such entry: {u}"))
                        })
                        .collect::<anyhow::Result<_>>()?;
                    Ok((node, children))
                })
                .collect::<anyhow::Result<_>>()?,
            js: value
                .js
                .into_iter()
                .map(|(url, set)| {
                    let node = value
                        .url_bucket
                        .get(&url)
                        .cloned()
                        .ok_or_else(|| anyhow!("fatal: no such entry: {url}"))?;
                    let children = set
                        .into_iter()
                        .map(|u| {
                            value
                                .url_bucket
                                .get(&u)
                                .cloned()
                                .ok_or_else(|| anyhow!("fatal: no such entry: {u}"))
                        })
                        .collect::<anyhow::Result<_>>()?;
                    Ok((node, children))
                })
                .collect::<anyhow::Result<_>>()?,
            secrets: value
                .url_secrets
                .into_iter()
                .map(|(url, secrets)| {
                    let node = value
                        .url_bucket
                        .get(&url)
                        .cloned()
                        .ok_or_else(|| anyhow!("fatal: no such entry: {url}"))?;
                    Ok((node, secrets))
                })
                .collect::<anyhow::Result<_>>()?,
        })
    }
}
impl CrawlerResult {}
#[derive(Builder)]
#[builder(pattern = "owned", build_fn(skip, error = "CrawlerBuildError"))]
#[allow(unused)]
#[allow(missing_docs)]
/// Concurrent web crawler over seed URLs.
pub struct Crawler<F, H>
where
    F: URLFilter,
    H: Handler,
{
    seeds: Vec<String>,
    filter: F,
    parser: Arc<URLParser<H>>,
    rate_limiter: Arc<Mutex<DomainRateLimiter>>,
    secret_handler: Arc<H>,

    max_page_num: Option<u32>,
    #[builder(default=Some(3))]
    max_depth: Option<u32>,
    #[builder(default = false)]
    follow_redirects: bool,
    dangerous_paths: Option<Vec<String>>,
    #[builder(default = false)]
    validate: bool,
    proxy: Option<String>,
    headers: Option<HeaderMap>,
    #[builder(default = Duration::from_secs(5))]
    timeout: Duration,

    #[builder(setter(skip), default = "self.default_workers_addr()?")]
    workers_addr: Vec<Addr<Worker<H>>>,

    #[builder(setter(skip), default)]
    state: CrawlerState,
}

impl<Filter, H> CrawlerBuilder<Filter, H>
where
    Filter: URLFilter,
    H: Handler,
{
    fn default_workers_addr(&self) -> Result<Vec<Addr<Worker<H>>>> {
        let mut builder = Client::builder();
        if let Some(Some(proxy)) = self.proxy.as_ref() {
            builder = builder.proxy(Proxy::all(proxy)?);
        }
        if let Some(Some(custom_headers)) = self.headers.as_ref() {
            builder = builder.default_headers(custom_headers.clone());
        }
        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout);
        }
        if self.follow_redirects.unwrap_or_default() {
            builder = builder.redirect(Policy::limited(MAX_REDIRECT));
        } else {
            builder = builder.redirect(Policy::none());
        }
        let client = builder.build()?;

        (0..available_parallelism()?.get())
            .map(|_| -> Result<Addr<Worker<H>>> {
                Ok(WorkerBuilder::default()
                    .client(client.clone())
                    .parser(self.parser.clone().ok_or(anyhow!("parser is required"))?)
                    .rate_limiter(
                        self.rate_limiter
                            .clone()
                            .ok_or(anyhow!("rate_limiter is required"))?,
                    )
                    .handler(
                        self.secret_handler
                            .clone()
                            .ok_or(anyhow!("handler is required"))?,
                    )
                    .build()?
                    .start())
            })
            .collect::<Result<Vec<Addr<Worker<H>>>>>()
    }
    /// Build a crawler from configured builder fields.
    pub fn build(self) -> Result<Crawler<Filter, H>, String> {
        let workers_addr = self.default_workers_addr().map_err(|e| e.to_string())?;
        let mut crawler = Crawler {
            seeds: self.seeds.ok_or("seeds is required")?,
            filter: self.filter.ok_or("filter is required")?,
            parser: self.parser.ok_or("parser is required")?,
            rate_limiter: self.rate_limiter.ok_or("rate_limiter is required")?,
            secret_handler: self.secret_handler.ok_or("handler is required")?,
            max_page_num: self.max_page_num.flatten(),
            max_depth: self.max_depth.flatten().or(Some(3)),
            follow_redirects: self.follow_redirects.unwrap_or(false),
            dangerous_paths: self.dangerous_paths.flatten(),
            validate: self.validate.unwrap_or(false),
            proxy: self.proxy.flatten(),
            headers: self.headers.flatten(),
            timeout: self.timeout.unwrap_or(Duration::from_secs(5)),
            workers_addr,
            state: CrawlerState::default(),
        };
        if let Some(dangerous_paths) = crawler.dangerous_paths {
            let i = dangerous_paths
                .iter()
                .filter_map(|p| {
                    if p.starts_with("/") {
                        None
                    } else {
                        Some(format!("/{p}").to_string())
                    }
                })
                .collect::<Vec<String>>();
            crawler.dangerous_paths = Some(dangerous_paths.into_iter().chain(i).collect());
        }

        Ok(crawler)
    }
}

impl<Filter, H> Crawler<Filter, H>
where
    Filter: URLFilter,
    H: Handler,
{
    /// Run the crawl until queues are exhausted or limits are reached.
    pub async fn run(&mut self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel::<Result<ScrapeResult, String>>(MAX_INFLIGHT_TASKS);
        let mut next_worker = 0;

        // queue seeds
        self.seeds.iter().try_for_each(|u| -> Result<()> {
            let node = URLNodeBuilder::default()
                .url(u.to_string())
                .depth(0)
                .build()?;
            self.filter.filter(&node.url_obj).then(|| {
                self.state.visited_urls.insert(node.url_obj.clone());
                self.state.working_queue.push_back(node.url_obj.clone());
                self.state.url_bucket.insert(node.url_obj.clone(), node);
            });
            Ok(())
        })?;
        loop {
            // producer
            while let Some(url) = self.state.working_queue.pop_front() {
                if let Some(m) = self.max_page_num
                    && self.state.page_cnt >= m
                {
                    break;
                }
                if let Some(url_node) = self.state.url_bucket.get(&url) {
                    if let Some(m) = self.max_depth
                        && url_node.depth > m
                    {
                        continue;
                    }
                    if self.should_evade(url_node.url_obj.path()) {
                        continue;
                    }
                    let addr = self.workers_addr[next_worker % self.workers_addr.len()].clone();
                    let tx2 = tx.clone();
                    task::spawn(async move {
                        let res: Result<ScrapeResult, String> = addr
                            .send(ScrapeMessage { url })
                            .await
                            .map_err(|e| e.to_string());
                        let _ = tx2.send(res).await;
                    });
                    self.state.in_flight += 1;
                    self.state.page_cnt += 1;
                    next_worker += 1;
                    if self.state.in_flight >= MAX_INFLIGHT_TASKS {
                        break;
                    }
                }
            }
            // end point
            let reached_page_limit = self
                .max_page_num
                .is_some_and(|max_page| self.state.page_cnt >= max_page);
            if self.state.in_flight == 0
                && (self.state.working_queue.is_empty() || reached_page_limit)
            {
                break;
            }
            // print per result
            debug!(
                "Total:{}, Found:{}, Inflight:{} Visited:{}, Secrets:{}",
                self.state.page_cnt,
                self.state.url_bucket.len(),
                self.state.in_flight,
                self.state.visited_urls.len(),
                self.state
                    .url_secrets
                    .values()
                    .map(|v| v.len())
                    .sum::<usize>(),
            );
            // consumer: consume one result
            if let Some(send_result) = rx.recv().await {
                match send_result {
                    Ok(result) => {
                        // record result and extend
                        self.consume(result);
                    }
                    Err(send_err) => {
                        return Err(anyhow!("dispatch scrape task error: {send_err}"));
                    }
                }
            }
        }
        if self.validate {
            self.validate().await?;
        }
        Ok(())
    }
    /// Consume the crawler and return aggregated results.
    pub fn result(self) -> Result<CrawlerResult> {
        CrawlerResult::try_from(self.state)
    }
    fn consume(&mut self, result: ScrapeResult) {
        self.state.in_flight -= 1;
        match result {
            ScrapeResult::Ignore(url) => {
                debug!("ignored: {}", url.url);
                if let Some(u) = self.state.url_bucket.get_mut(&url.url_obj) {
                    u.response_status = ResponseStatus::Ignore;
                }
            }
            ScrapeResult::Ok(result) => {
                enum ResultType {
                    JS,
                    Url,
                }
                info!(
                    "{}: {} url children, {} js children, {} secrets",
                    result.url.url,
                    result.url_children.len(),
                    result.js_children.len(),
                    result.secrets.len()
                );
                if let Some(node) = self.state.url_bucket.get_mut(&result.url.url_obj) {
                    node.response_status = result.url.response_status;
                    node.content_length = result.url.content_length;
                    node.content_type = result.url.content_type;
                    node.title = result.url.title;
                }

                // extend
                result
                    .url_children
                    .into_iter()
                    .map(|u| (ResultType::Url, u))
                    .chain(result.js_children.into_iter().map(|u| (ResultType::JS, u)))
                    .for_each(|(t, url)| {
                        self.state
                            .url_bucket
                            .insert(url.url_obj.clone(), url.clone());
                        match t {
                            ResultType::Url => {
                                self.state
                                    .urls
                                    .entry(result.url.url_obj.clone())
                                    .or_default()
                                    .insert(url.url_obj.clone());
                            }
                            ResultType::JS => {
                                self.state
                                    .js
                                    .entry(result.url.url_obj.clone())
                                    .or_default()
                                    .insert(url.url_obj.clone());
                            }
                        }
                        let is_legal_depth = match self.max_depth {
                            Some(m) => url.depth <= m,
                            None => true,
                        };
                        debug!("New link found: {} from {}", url.url, result.url.url);
                        if !self.state.visited_urls.contains(&url.url_obj)
                            && is_legal_depth
                            && self.filter.filter(&url.url_obj)
                        {
                            // enqueue
                            self.state.visited_urls.insert(url.url_obj.clone());
                            self.state.working_queue.push_back(url.url_obj);
                        }
                    });
                // save secrets
                self.state
                    .url_secrets
                    .entry(result.url.url_obj)
                    .or_default()
                    .extend(result.secrets);
            }
            ScrapeResult::Err(err) => match err {
                ScrapeError::FetchError { url, err } => {
                    warn!("fail to fetch {url}: {err}");
                    if let Some(u) = self.state.url_bucket.get_mut(&url) {
                        u.response_status = ResponseStatus::Failed(err.to_string());
                    }
                }
                ScrapeError::ProcessError { url, err } => {
                    error!("process error: {err}");
                    if let Some(u) = self.state.url_bucket.get_mut(&url) {
                        u.response_status = ResponseStatus::Failed(err.to_string());
                    }
                }
            },
        }
    }

    fn should_evade(&self, path: &str) -> bool {
        if let Some(dangerous_paths) = &self.dangerous_paths
            && dangerous_paths.iter().any(|p| path.contains(p))
        {
            true
        } else {
            false
        }
    }
    /// Validate the status of scraped urls that are marked as unknown
    async fn validate(&mut self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel::<Result<FetchResult, String>>(MAX_INFLIGHT_TASKS);
        let unknown_urls = self
            .state
            .url_bucket
            .iter()
            .filter_map(|(url, node)| {
                if matches!(node.response_status, ResponseStatus::Unknown) {
                    return Some(url);
                }
                None
            })
            .map(Url::clone)
            .collect::<Vec<Url>>();
        let mut unknown_urls = unknown_urls.iter();
        let mut in_flight = 0usize;
        let mut next_worker = 0usize;
        loop {
            for url in unknown_urls.by_ref() {
                if self.should_evade(url.path()) {
                    if let Some(node) = self.state.url_bucket.get_mut(url) {
                        node.response_status = ResponseStatus::Ignore;
                    }
                    continue;
                }
                if in_flight >= MAX_INFLIGHT_TASKS {
                    break;
                }
                let worker = self.workers_addr[next_worker % self.workers_addr.len()].clone();
                let tx = tx.clone();
                let url = url.clone();
                task::spawn(async move {
                    let res = worker
                        .send(FetchMessage { url })
                        .await
                        .map_err(|e| e.to_string());
                    let _ = tx.send(res).await.map_err(|e| e.to_string());
                    Ok::<(), String>(())
                });
                in_flight += 1;
                next_worker += 1;
            }
            if in_flight == 0 {
                break;
            }
            // consume
            if let Some(send_result) = rx.recv().await {
                in_flight -= 1;
                match send_result {
                    Ok(result) => {
                        match result {
                            FetchResult::Success(result) => {
                                self
                                    // update
                                    .state
                                    .url_bucket
                                    .entry(result.url_obj)
                                    .and_modify(|node| {
                                        node.response_status = result.response_status;
                                        node.content_length = result.content_length;
                                        node.content_type = result.content_type;
                                        node.title = result.title;
                                    });
                            }
                            FetchResult::Err(e) => {
                                error!("validate error: {e}");
                                match e {
                                    ScrapeError::FetchError { url, err } => {
                                        if let Some(node) = self.state.url_bucket.get_mut(&url) {
                                            node.response_status =
                                                ResponseStatus::Failed(err.to_string());
                                        }
                                    }
                                    ScrapeError::ProcessError { url, err } => {
                                        if let Some(node) = self.state.url_bucket.get_mut(&url) {
                                            node.response_status =
                                                ResponseStatus::Failed(err.to_string());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(send_err) => {
                        return Err(anyhow!("dispatch scrape task error: {send_err}"));
                    }
                }
            }
        }

        Ok(())
    }
}
