//! Actix worker actor for fetching URLs and extracting artifacts.

use std::{collections::HashSet, sync::Arc};

use actix::{Actor, Context, ResponseFuture};
use derive_builder::Builder;
use lazy_static::lazy_static;
use reqwest::{Client, header};
use scraper::Selector;
use tokio::sync::{Mutex, oneshot};
use url::Url;

use crate::{
    handler::Handler,
    rate_limiter::DomainRateLimiter,
    scraper::bo::{
        FetchMessage, FetchResult, ScrapeArtifacts, ScrapeError, ScrapeMessage, ScrapeResult,
        ScrapeStdResult,
    },
    urlparser::{ResponseStatus, URLNode, URLNodeBuilder, URLParser, response_title},
};
lazy_static! {
    static ref title_selector: Selector = Selector::parse("title").unwrap();
}

/// Worker actor fetch single url and extract secrets and children
#[derive(Builder)]
#[builder(pattern = "owned")]
#[allow(missing_docs)]
pub struct Worker<H: Handler> {
    client: Client,
    handler: Arc<H>,
    parser: Arc<URLParser<H>>,
    rate_limiter: Arc<Mutex<DomainRateLimiter>>,
}
impl<H: Handler> Clone for Worker<H> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            handler: self.handler.clone(),
            parser: self.parser.clone(),
            rate_limiter: self.rate_limiter.clone(),
        }
    }
}
impl<H: Handler> Actor for Worker<H> {
    type Context = Context<Self>;
}
impl<H: Handler> actix::Handler<ScrapeMessage> for Worker<H> {
    type Result = ResponseFuture<ScrapeResult>;
    fn handle(&mut self, msg: ScrapeMessage, _ctx: &mut Self::Context) -> Self::Result {
        let worker = self.clone();
        Box::pin(async move { worker.scrape(msg.url).await })
    }
}
impl<H: Handler> actix::Handler<FetchMessage> for Worker<H> {
    type Result = ResponseFuture<FetchResult>;
    fn handle(&mut self, msg: FetchMessage, _ctx: &mut Self::Context) -> Self::Result {
        let worker = self.clone();
        Box::pin(async move {
            let mut url;
            match URLNodeBuilder::default().url(msg.url.to_string()).build() {
                Err(e) => {
                    return FetchResult::Err(ScrapeError::process_error(
                        msg.url,
                        format!("URLNodeBuilder error: {e:?}"),
                    ));
                }
                Ok(u) => {
                    url = u;
                }
            }
            match worker.fetch(&mut url).await {
                Err(err) => FetchResult::Err(err),
                Ok(_) => FetchResult::Success(url),
            }
        })
    }
}
enum ScrapeInnerResult {
    Normal(ScrapeArtifacts),
    Ignore(URLNode),
}
impl<H: Handler> Worker<H> {
    async fn scrape(self, url: Url) -> ScrapeResult {
        let node = match URLNodeBuilder::default().url(url.to_string()).build() {
            Err(e) => {
                return ScrapeResult::Err(ScrapeError::process_error(
                    url,
                    format!("URLNodeBuilder error: {e}"),
                ));
            }
            Ok(u) => u,
        };
        match self.scrape_inner(node).await {
            Ok(artifacts) => match artifacts {
                ScrapeInnerResult::Normal(artifacts) => ScrapeResult::Ok(artifacts),
                ScrapeInnerResult::Ignore(node) => ScrapeResult::Ignore(node),
            },
            Err(e) => ScrapeResult::Err(e),
        }
    }
    /// return: response body, ignored or not
    async fn fetch(&self, url: &mut URLNode) -> ScrapeStdResult<(String, bool)> {
        let _permit = {
            let mut guard = self.rate_limiter.lock().await;
            guard
                .acquire(url.url_obj.host_str().unwrap_or_default().to_string())
                .await
                .map_err(|e| {
                    url.response_status = ResponseStatus::Failed(format!("{:?}", e));
                    ScrapeError::process_error(
                        url.url_obj.clone(),
                        format!("rate limiter acquire error: {e:?}"),
                    )
                })?
        };
        let resp = self.client.get(url.url.clone()).send().await.map_err(|e| {
            url.response_status = ResponseStatus::Failed(e.to_string());
            ScrapeError::fetch_error(url.url_obj.clone(), e)
        })?;
        url.content_type = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        if let Some(content_type) = &url.content_type
            && !should_process(content_type)
        {
            return Ok(("".to_string(), true));
        }

        url.response_status = ResponseStatus::Valid(resp.status().as_u16());
        url.content_length = resp.content_length();
        let html = resp.text().await.map_err(|e| {
            ScrapeError::process_error(
                url.url_obj.clone(),
                format!("{} receive response body: {e:?}", &url.url),
            )
        })?;
        if let Some(ct) = &url.content_type
            && is_html(ct)
        {
            url.title = response_title(&html).ok();
        }
        Ok((html, false))
    }
    async fn scrape_inner(&self, url: URLNode) -> ScrapeStdResult<ScrapeInnerResult> {
        let mut url_owned = url;
        let url = &mut url_owned;
        // fetch
        let (html, ignored) = self.fetch(url).await?;
        if ignored {
            return Ok(ScrapeInnerResult::Ignore(url.clone()));
        }

        // extract children links
        let mut js_children = HashSet::new();
        let mut url_children = HashSet::new();
        self.parser
            .extract_urls(url, &html)
            .map_err(|e| {
                ScrapeError::process_error(
                    url.url_obj.clone(),
                    format!("{} extract urls: {e:?}", &url.url),
                )
            })?
            .into_iter()
            .for_each(|u| {
                if is_js(&u) {
                    js_children.insert(u);
                } else {
                    url_children.insert(u);
                }
            });
        // extract secrets
        let (tx, rx) = oneshot::channel();
        let handler = self.handler.clone();
        rayon::spawn(move || {
            let out = handler.handle(&html);
            let _ = tx.send(out);
        });
        let secrets = rx
            .await
            .map_err(|e| {
                ScrapeError::process_error(
                    url.url_obj.clone(),
                    format!("{} rayon task cancelled: {e:?}", &url.url),
                )
            })?
            .map_err(|e| {
                ScrapeError::process_error(
                    url.url_obj.clone(),
                    format!("{} extract secrets: {e:?}", &url.url),
                )
            })?;

        Ok(ScrapeInnerResult::Normal(ScrapeArtifacts {
            url: url_owned,
            secrets: HashSet::from_iter(secrets),
            js_children,
            url_children,
        }))
    }
}
fn is_js(url: &URLNode) -> bool {
    let path = url.url_obj.path().to_lowercase();
    path.ends_with(".js") || path.ends_with(".js.map")
}
fn should_process(content_type: &str) -> bool {
    let mut content_type = content_type.to_lowercase();
    if let Some((c, _)) = content_type.split_once(";") {
        content_type = c.to_string();
    }
    content_type.starts_with("text/")
        || matches!(
            content_type.as_str(),
            "application/json"
                | "application/ld+json"
                | "application/javascript"
                | "application/x-javascript"
                | "application/xml"
                | "application/xhtml+xml"
                | "application/x-www-form-urlencoded"
        )
}
fn is_html(content_type: &str) -> bool {
    content_type
        .split(";")
        .next()
        .map(str::trim)
        .unwrap_or_default()
        .eq_ignore_ascii_case("text/html")
}
