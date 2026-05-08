use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    thread::available_parallelism,
};

use actix::{Actor, Addr};
use anyhow::{Result, anyhow};
use derive_builder::Builder;
use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use reqwest::redirect::Policy;
use reqwest::{Client, Proxy};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio::task;
use tracing::{debug, error, warn};

use crate::scraper::bo::ScrapeError;
use crate::scraper::worker::{Worker, WorkerBuilder};
use crate::urlparser::{URLNode, URLNodeBuilder, URLParser};
use crate::{filter::URLFilter, handler::Handler};
use crate::{handler::Secret, scraper::bo::ScrapeMessage};
use crate::{rate_limiter::DomainRateLimiter, scraper::bo::ScrapeResult};
use derive_builder::UninitializedFieldError;
use std::time::Duration;

static MAX_INFLIGHT_TASKS: usize = 256;
static MAX_REDIRECT: usize = 5;

#[derive(Debug)]
pub enum CrawlerBuildError {
    UninitializedFieldError(String),
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
    visited_urls: HashSet<URLNode>,
    found_urls: HashSet<URLNode>,
    working_queue: VecDeque<URLNode>,
    urls: HashMap<URLNode, HashSet<URLNode>>,
    js: HashMap<URLNode, HashSet<URLNode>>,
    page_cnt: u32,
    url_secrets: HashMap<URLNode, HashSet<Secret>>,
}
#[derive(Builder)]
#[builder(pattern = "owned", build_fn(skip, error = "CrawlerBuildError"))]
pub struct Crawler<'a, F, H>
where
    F: URLFilter,
    H: Handler,
{
    runtime: &'a Runtime,
    seeds: Vec<String>,
    filter: F,
    parser: Arc<URLParser<H>>,
    rate_limiter: DomainRateLimiter<'a>,
    secret_handler: Arc<H>,

    #[builder(setter(strip_option))]
    max_page_num: Option<u32>,
    #[builder(default=Some(3), setter(strip_option))]
    max_depth: Option<u32>,
    #[builder(default = false)]
    follow_redirects: bool,
    #[builder(setter(strip_option))]
    dangerous_paths: Option<Vec<String>>,
    #[builder(default = false)]
    validate: bool,
    #[builder(setter(strip_option))]
    proxy: Option<String>,
    #[builder(setter(strip_option))]
    headers: Option<HeaderMap>,
    #[builder(default = Duration::from_secs(5))]
    timeout: Duration,

    #[builder(setter(skip), default = "self.default_workers_addr()?")]
    workers_addr: Vec<Addr<Worker<H>>>,

    #[builder(setter(skip), default)]
    state: CrawlerState,
}

impl<'a, Filter, H> CrawlerBuilder<'a, Filter, H>
where
    Filter: URLFilter,
    H: Handler,
{
    fn default_workers_addr(&self) -> Result<Vec<Addr<Worker<H>>>> {
        let mut builder = Client::builder();
        if let Some(Some(proxy)) = self.proxy.as_ref() {
            builder = builder.proxy(Proxy::all(proxy)?);
        }
        let mut headers = HeaderMap::new();
        // default headers
        headers.insert(USER_AGENT,HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/80.0.3987.87 Safari/537.36 SE 2.X MetaSr 1.0"));
        headers.insert(ACCEPT, HeaderValue::from_static("*/*"));

        if let Some(Some(custom_headers)) = self.headers.as_ref() {
            headers.extend(custom_headers.clone());
        }
        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout);
        }
        if self.follow_redirects.unwrap_or_default() {
            builder = builder.redirect(Policy::limited(MAX_REDIRECT));
        }
        let client = builder.build()?;

        (0..available_parallelism()?.get())
            .map(|_| -> Result<Addr<Worker<H>>> {
                Ok(WorkerBuilder::default()
                    .client(client.clone())
                    .parser(self.parser.clone().ok_or(anyhow!("parser is required"))?)
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
    pub fn build(self) -> Result<Crawler<'a, Filter, H>, String> {
        let workers_addr = self.default_workers_addr().map_err(|e| e.to_string())?;
        Ok(Crawler {
            runtime: self.runtime.as_ref().ok_or("runtime is required")?,
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
        })
    }
}

impl<'a, Filter, H> Crawler<'a, Filter, H>
where
    Filter: URLFilter,
    H: Handler,
{
    pub async fn run(&mut self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel::<Result<ScrapeResult, String>>(MAX_INFLIGHT_TASKS);
        let next_worker = 0;

        // queue seeds
        self.seeds.iter().try_for_each(|u| -> Result<()> {
            let node = URLNodeBuilder::default()
                .url(u.to_string())
                .depth(0)
                .build()?;
            self.filter.filter(&node.url_obj).then(|| {
                self.state.visited_urls.insert(node.clone());
                self.state.working_queue.push_back(node);
            });
            Ok(())
        })?;
        loop {
            // producer
            while !self.state.working_queue.is_empty() {
                if let Some(m) = self.max_page_num
                    && self.state.page_cnt >= m
                {
                    break;
                }
                let depth;
                if let Some(url) = self.state.working_queue.pop_front() {
                    depth = url.depth;
                    if let Some(m) = self.max_depth
                        && url.depth > m
                    {
                        continue;
                    }
                    if self.should_evade(&url) {
                        continue;
                    }
                    let addr = self.workers_addr[next_worker % self.workers_addr.len()].clone();
                    let tx2 = tx.clone();
                    task::spawn(async move {
                        // no shortcircuit, full Result comes into channel
                        let res: Result<ScrapeResult, String> = addr
                            .send(ScrapeMessage { url })
                            .await
                            .map_err(|e| e.to_string());
                        let _ = tx2.send(res).await.map_err(|e| e.to_string());
                        Ok::<(), String>(())
                    });
                    self.state.in_flight += 1;
                    self.state.page_cnt += 1;
                }
            }
            // end point
            if self.state.in_flight == 0 && self.state.working_queue.is_empty() {
                break;
            }
            // print per result
            debug!(
                "Total:{}, Found:{}, Inflight:{} Visited:{}, Secrets:{}",
                self.state.page_cnt,
                self.state.found_urls.len(),
                self.state.in_flight,
                self.state.visited_urls.len(),
                self.state
                    .url_secrets.values().map(|v| v.len())
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
                        panic!("dispatch scrape task error: {send_err}");
                    }
                }
            }
        }
        Ok(())
    }
    fn consume(&mut self, result: ScrapeResult) {
        match result {
            ScrapeResult::Ignore(url) => debug!("ignored: {}", url.url),
            ScrapeResult::Ok(result) => {
                enum ResultType {
                    JS,
                    Url,
                }
                let _ = result
                    .url_children
                    .into_iter()
                    .map(|u| (ResultType::Url, u))
                    .chain(result.js_children.into_iter().map(|u| (ResultType::JS, u)))
                    .map(|(t, url)| {
                        self.state.found_urls.insert(url.clone());
                        match t {
                            ResultType::Url => {
                                self.state
                                    .urls
                                    .entry(result.url.clone())
                                    .or_default()
                                    .insert(url.clone());
                            }
                            ResultType::JS => {
                                self.state
                                    .js
                                    .entry(result.url.clone())
                                    .or_default()
                                    .insert(url.clone());
                            }
                        }
                        let is_legal_depth = match self.max_depth {
                            Some(m) => url.depth < m,
                            None => true,
                        };
                        debug!("New link found: {} from {}", url.url, result.url.url);
                        if !self.state.visited_urls.contains(&url)
                            && is_legal_depth
                            && self.filter.filter(&url.url_obj)
                        {
                            // enqueue
                            self.state.visited_urls.insert(url.clone());
                            self.state.working_queue.push_back(url);
                        }
                    })
                    .collect::<Vec<_>>();
            }
            ScrapeResult::Err(err) => match err {
                ScrapeError::FetchError { url, err } => {
                    warn!("fail to fetch {url}: {err}");
                }
                ScrapeError::ProcessError(err) => {
                    error!("process error: {err}")
                }
            },
        }
    }

    fn should_evade(&self, url: &URLNode) -> bool {
        if let Some(dangerous_paths) = &self.dangerous_paths
            && dangerous_paths.iter().any(|p| {
                url.url_obj.path().contains(&format!("/{p}")) || url.url_obj.path().contains(p)
            })
        {
            true
        } else {
            false
        }
    }
    /// Validate the status of scraped urls that are marked as unknown
    async fn validate(&mut self) {
        todo!("impl")
    }
}
