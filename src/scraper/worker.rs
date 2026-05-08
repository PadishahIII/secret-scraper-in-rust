use std::fmt::Error;

use actix::{Actor, Context, ResponseFuture, WrapFuture};
use derive_builder::Builder;
use lazy_static::lazy_static;
use reqwest::{Client, Response, header};
use scraper::{Html, Selector};

use crate::{
    handler::Handler,
    scraper::bo::{ScrapeArtifacts, ScrapeError, ScrapeMessage, ScrapeResult, ScrapeStdResult},
    urlparser::{ResponseStatus, URLNode},
};
lazy_static! {
    static ref title_selector: Selector = Selector::parse("title").unwrap();
}

/// Worker actor fetch single url and extract secrets and children
#[derive(Builder, Clone)]
pub struct Worker<H: Handler + Unpin + 'static> {
    client: Client,
    handler: H,
}
impl<H: Handler + Unpin + 'static> Actor for Worker<H> {
    type Context = Context<Self>;
}
impl<H: Handler + Unpin + 'static> actix::Handler<ScrapeMessage> for Worker<H> {
    type Result = ResponseFuture<ScrapeResult>;
    fn handle(&mut self, msg: ScrapeMessage, ctx: &mut Self::Context) -> Self::Result {
        let worker = self.clone();
        Box::pin(async move { worker.scrape(msg.url).await })
    }
}
enum ScrapeInnerResult {
    Normal(ScrapeArtifacts),
    Ignore(URLNode),
}
impl<H: Handler + Unpin + 'static> Worker<H> {
    async fn scrape(self, url: URLNode) -> ScrapeResult {
        match self.scrape_inner(url).await {
            Ok(artifacts) => match artifacts {
                ScrapeInnerResult::Normal(artifacts) => ScrapeResult::Ok(artifacts),
                ScrapeInnerResult::Ignore(url) => ScrapeResult::Ignore(url),
            },
            Err(e) => ScrapeResult::Err(e),
        }
    }
    async fn scrape_inner(&self, url: URLNode) -> ScrapeStdResult<ScrapeInnerResult> {
        // fetch
        let resp = self.client.get(url.url).send().await.map_err(|e| {
            url.response_status = ResponseStatus::Failed(e.to_string());
            ScrapeError::fetch_error(url.url, e)
        })?;
        url.response_status = ResponseStatus::Valid(resp.status().as_u16());
        url.content_length = resp.content_length();
        url.content_type = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let html = resp.text().await?;
        let doc = Html::parse_document(&html);
        url.title = doc
            .select(&title_selector)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string());

        // extract
        let secrets = self.handler.handle(&html)?;

        ScrapeInnerResult::Normal(ScrapeArtifacts { url })
    }
}
