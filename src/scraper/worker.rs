use actix::{Actor, Context, Handler};
use derive_builder::Builder;
use reqwest::Client;

use crate::scraper::crawler::{ScrapeMessage, ScrapeResult};

/// Worker actor fetch single url and extract secrets and children
#[derive(Builder)]
struct Worker {
    client: Client,
}
impl Actor for Worker {
    type Context = Context<Self>;
}
impl Handler<ScrapeMessage> for Worker {
    type Result = ScrapeResult;
    fn handle(&mut self, msg: ScrapeMessage, ctx: &mut Self::Context) -> Self::Result {
        todo!("impl")
    }
}
