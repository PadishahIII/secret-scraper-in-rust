use std::collections::HashSet;

use actix::{Message, MessageResponse};

use crate::{handler::Secret, urlparser::URLNode};

#[derive(Message)]
#[rtype(result = "ScrapeResult")]
pub struct ScrapeMessage {
    pub url: URLNode,
}

pub struct ScrapeArtifacts {
    pub url: URLNode,
    pub secrets: HashSet<Secret>,
    pub js_children: HashSet<URLNode>,
    pub url_children: HashSet<URLNode>,
}
#[derive(Debug)]
pub enum ScrapeError {
    FetchError { url: String, err: reqwest::Error },
    ProcessError(String),
}
impl ScrapeError {
    pub fn fetch_error(url: String, err: reqwest::Error) -> Self {
        Self::FetchError { url, err }
    }
    pub fn process_error(err: String) -> Self {
        Self::ProcessError(err)
    }
}
pub type ScrapeStdResult<T> = Result<T, ScrapeError>;

#[derive(MessageResponse)]
pub enum ScrapeResult {
    Ok(ScrapeArtifacts),
    /// the response is non-processable (not text response for example)
    Ignore(URLNode),
    Err(ScrapeError),
}
