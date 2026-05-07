use std::collections::{HashMap, HashSet};

use derive_builder::Builder;
use reqwest::Response;
use reqwest::header::HeaderMap;
use tokio::runtime::Runtime;

use crate::handler::Secret;
use crate::urlparser::{URLNode, URLParser};
use crate::{filter::URLFilter, handler::Handler};

#[derive(Default)]
struct CrawlerState<'a> {
    visited_urls: HashSet<URLNode<'a>>,
    found_urls: HashSet<URLNode<'a>>,
    // TODO:working_queue
    urls: HashMap<URLNode<'a>, HashSet<URLNode<'a>>>,
    js: HashMap<URLNode<'a>, HashSet<URLNode<'a>>>,
    page_cnt: u32,
    url_secrets: HashMap<URLNode<'a>, HashSet<Secret>>,
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
    max_page_num: Option<u32>,
    #[builder(default=Some(3))]
    max_depth: Option<u32>,
    #[builder(default=Some(100))]
    max_connections: Option<u32>,
    #[builder(default=Some(50))]
    max_keepalive_conns: Option<u32>,
    #[builder(default=Some(5))]
    max_concurrency_per_domain: Option<u32>,
    #[builder(default=Some(0.2))]
    min_request_interval: Option<f32>,
    #[builder(default = false)]
    follow_redirects: bool,
    dangerous_paths: Option<Vec<String>>,
    #[builder(default = false)]
    validate: bool,
    proxy: Option<String>,
    headers: Option<HeaderMap>,
    #[builder(default = 5.0)]
    timeout: f32,
    #[builder(setter(skip))]
    state: CrawlerState<'a>,
    // TODO: rate limiter
}

impl<'a, Filter, H> Crawler<'a, Filter, H>
where
    Filter: URLFilter,
    H: Handler,
{
    pub async fn run(&mut self) {
        todo!("impl")
    }
    /// Validate the status of scraped urls that are marked as unknown
    async fn validate(&mut self) {
        todo!("impl")
    }
    async fn extract_and_extend(&mut self, _base: &URLNode<'a>, _response: &Response) {
        todo!("impl")
    }
    async fn fetch(&self, _url: &str) -> Response {
        todo!("impl")
    }
}
