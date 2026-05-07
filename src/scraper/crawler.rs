use std::collections::{HashMap, HashSet};

use actix::{Message, MessageResponse};
use anyhow::Result;
use derive_builder::Builder;
use reqwest::Response;
use reqwest::header::HeaderMap;
use tokio::runtime::Runtime;
use tracing::debug;

use crate::handler::Secret;
use crate::rate_limiter::DomainRateLimiter;
use crate::urlparser::{URLNode, URLNodeBuilder, URLParser};
use crate::{filter::URLFilter, handler::Handler};

#[derive(Message)]
#[rtype(result = "ScrapeResult")]
pub struct ScrapeMessage {
    url: URLNode,
}
#[derive(MessageResponse)]
pub struct ScrapeResult {
    url: URLNode,
    secrets: HashSet<Secret>,
    js_children: HashSet<URLNode>,
    url_children: HashSet<URLNode>,
}

#[derive(Default)]
struct CrawlerState {
    visited_urls: HashSet<URLNode>,
    found_urls: HashSet<URLNode>,
    working_queue: Vec<URLNode>,
    urls: HashMap<URLNode, HashSet<URLNode>>,
    js: HashMap<URLNode, HashSet<URLNode>>,
    page_cnt: u32,
    url_secrets: HashMap<URLNode, HashSet<Secret>>,
}
#[derive(Builder)]
#[builder(pattern = "owned")]
pub struct Crawler<'a, F, H>
where
    F: URLFilter,
    H: Handler,
{
    runtime: &'a Runtime,
    seeds: Vec<String>,
    filter: F,
    parser: URLParser<H>,
    rate_limiter: DomainRateLimiter<'a>,

    max_page_num: Option<u32>,
    #[builder(default=Some(3))]
    max_depth: Option<u32>,
    #[builder(default=Some(100))]
    max_connections: Option<u32>,
    #[builder(default=Some(50))]
    max_keepalive_conns: Option<u32>,
    #[builder(default = false)]
    follow_redirects: bool,
    dangerous_paths: Option<Vec<String>>,
    #[builder(default = false)]
    validate: bool,
    proxy: Option<String>,
    headers: Option<HeaderMap>,
    #[builder(default = 5.0)]
    timeout: f32,

    #[builder(setter(skip), default)]
    state: CrawlerState,
}

impl<'a, Filter, H> Crawler<'a, Filter, H>
where
    Filter: URLFilter,
    H: Handler,
{
    pub async fn run(&mut self) -> Result<()> {
        // queue seeds
        self.seeds.iter().try_for_each(|u| -> Result<()> {
            let node = URLNodeBuilder::default()
                .url(u.to_string())
                .depth(0)
                .build()?;
            self.filter.filter(&node.url_obj).then(|| {
                self.state.visited_urls.insert(node.clone());
                self.state.working_queue.push(node);
            });
            Ok(())
        })?;
        while !self.state.working_queue.is_empty() {
            if let Some(m) = self.max_page_num
                && self.state.page_cnt >= m
            {
                break;
            }
            let depth;
            match self.state.working_queue.pop() {
                Some(url) => {
                    depth = url.depth;
                    if let Some(m) = self.max_depth
                        && url.depth > m
                    {
                    } else {
                        self.process(url).await;
                    }
                }
                None => {
                    break;
                }
            }
            debug!(
                "Total:{}, Found:{}, Depth:{}, Visited:{}, Secrets:{}",
                self.state.page_cnt,
                self.state.found_urls.len(),
                depth,
                self.state.visited_urls.len(),
                self.state
                    .url_secrets
                    .iter()
                    .map(|(k, v)| v.len())
                    .sum::<usize>(),
            );
        }
        Ok(())
    }
    async fn process(&mut self, url: URLNode) {}
    async fn should_evade(&self, url: &URLNode) -> bool {
        todo!("impl")
    }
    /// Validate the status of scraped urls that are marked as unknown
    async fn validate(&mut self) {
        todo!("impl")
    }
    async fn extract_and_extend(&mut self, _base: &URLNode, _response: &Response) {
        todo!("impl")
    }
    async fn fetch(&self, _url: &str) -> Response {
        todo!("impl")
    }
}
