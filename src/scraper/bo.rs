use std::{collections::HashSet, fmt::Display};

use actix::{Message, MessageResponse};
use url::Url;

use crate::{handler::Secret, urlparser::URLNode};

#[derive(Message)]
#[rtype(result = "ScrapeResult")]
pub struct ScrapeMessage {
    pub url: Url,
}

pub struct ScrapeArtifacts {
    pub url: URLNode,
    pub secrets: HashSet<Secret>,
    pub js_children: HashSet<URLNode>,
    pub url_children: HashSet<URLNode>,
}

#[derive(Debug)]
pub enum ScrapeError {
    FetchError { url: Url, err: reqwest::Error },
    ProcessError { url: Url, err: String },
}
impl Display for ScrapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScrapeError::FetchError { url, err } => {
                write!(f, "fetch {} error: {err}", url)
            }
            ScrapeError::ProcessError { url, err } => {
                write!(f, "process {} error: {err}", url)
            }
        }
    }
}
impl ScrapeError {
    pub fn fetch_error(url: Url, err: reqwest::Error) -> Self {
        Self::FetchError { url, err }
    }
    pub fn process_error(url: Url, err: String) -> Self {
        Self::ProcessError { url, err }
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

#[derive(Message)]
#[rtype(result = "FetchResult")]
pub struct FetchMessage {
    pub url: Url,
}

#[derive(MessageResponse)]
pub enum FetchResult {
    Success(URLNode),
    Err(ScrapeError),
}
