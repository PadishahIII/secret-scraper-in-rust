//! Message, result, and error types used by crawler actors.

use std::{collections::HashSet, fmt::Display};

use actix::{Message, MessageResponse};
use url::Url;

use crate::{handler::Secret, urlparser::URLNode};

/// Message asking a worker to fetch and process one URL.
#[derive(Message)]
#[rtype(result = "ScrapeResult")]
pub struct ScrapeMessage {
    /// URL to scrape.
    pub url: Url,
}

/// Data extracted from a successfully scraped URL.
pub struct ScrapeArtifacts {
    /// Fetched URL node with response metadata.
    pub url: URLNode,
    /// Secrets found in the response body.
    pub secrets: HashSet<Secret>,
    /// JavaScript links discovered from the response body.
    pub js_children: HashSet<URLNode>,
    /// Non-JavaScript links discovered from the response body.
    pub url_children: HashSet<URLNode>,
}

/// Error returned while fetching or processing a URL.
#[derive(Debug)]
pub enum ScrapeError {
    /// HTTP fetch failed.
    FetchError {
        /// URL being fetched.
        url: Url,
        /// Fetch error from reqwest.
        err: reqwest::Error,
    },
    /// URL processing failed after or around fetch.
    ProcessError {
        /// URL being processed.
        url: Url,
        /// Processing error message.
        err: String,
    },
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
    /// Construct a fetch error.
    pub fn fetch_error(url: Url, err: reqwest::Error) -> Self {
        Self::FetchError { url, err }
    }
    /// Construct a processing error.
    pub fn process_error(url: Url, err: String) -> Self {
        Self::ProcessError { url, err }
    }
}
/// Result alias for scrape operations.
pub type ScrapeStdResult<T> = Result<T, ScrapeError>;

/// Worker scrape response.
#[derive(MessageResponse)]
pub enum ScrapeResult {
    /// Scrape completed and produced artifacts.
    Ok(ScrapeArtifacts),
    /// the response is non-processable (not text response for example)
    Ignore(URLNode),
    /// Scrape failed.
    Err(ScrapeError),
}

/// Message asking a worker to fetch one URL and return metadata.
#[derive(Message)]
#[rtype(result = "FetchResult")]
pub struct FetchMessage {
    /// URL to fetch.
    pub url: Url,
}

/// Worker fetch response.
#[derive(MessageResponse)]
pub enum FetchResult {
    /// Fetch succeeded.
    Success(URLNode),
    /// Fetch failed.
    Err(ScrapeError),
}
