//! URL node representation and link extraction helpers.

use std::{collections::HashSet, fmt::Display, hash::Hash};

use anyhow::{Result, anyhow, bail};
use derive_builder::Builder;
use lazy_static::lazy_static;
use regex::Regex;
use scraper::{Html, Selector};
use serde::Serialize;
use url::Url;
use urlencoding::decode;
use urlparse::urlparse;

use crate::handler::Handler;

static STATIC_EXTS: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".gif", ".css", ".ico", ".dtd", ".svg", ".scss", ".vue", ".ts",
];
lazy_static! {
static ref IGNORED_URL:Regex = Regex::new("\\<|\\>|\\{|\\}|\\[|\\]|\\||\\^|;|/node_modules/|www\\.w3\\.org|example\\.com|jquery[-\\.\\w]*?\\.js|\\.src|\\.replace|\\.url|\\.att|\\.href|location\\.href|javascript:|location:|application/x-www-form-urlencoded|\\.createObject|:location|\\.path|\\*#__PURE__\\*|\\*\\$0\\*|\\n").unwrap();
static ref WORDS: Regex = Regex::new("[a-zA-Z0-9]+").unwrap();
}

/// Response status recorded for a URL node.
#[derive(Debug, Clone, Serialize)]
pub enum ResponseStatus {
    /// the url is not requested yet
    Unknown,
    /// Successful HTTP response status code.
    Valid(u16),
    /// invalid response status
    Failed(String),
    /// non-text response or dangerous path
    Ignore,
}
impl Display for ResponseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResponseStatus::Unknown => write!(f, "Unknown"),
            ResponseStatus::Valid(s) => write!(f, "{s}"),
            ResponseStatus::Failed(e) => write!(f, "Failed: {e}"),
            ResponseStatus::Ignore => write!(f, "Ignore"),
        }
    }
}

#[derive(Debug, Builder, Clone, Serialize)]
#[builder(default, build_fn(skip))]
#[allow(missing_docs)]
/// URL plus crawl metadata.
pub struct URLNode {
    /// String form of the URL.
    pub url: String,
    #[builder(setter(skip))]
    #[serde(skip)]
    /// Parsed URL object used for equality, hashing, and joins.
    pub url_obj: Url,
    #[builder(default=ResponseStatus::Unknown)]
    /// Response status observed for this URL.
    pub response_status: ResponseStatus,
    /// Crawl depth from the seed URL.
    pub depth: u32,
    /// Response content length, when known.
    pub content_length: Option<u64>,
    /// Response content type, when known.
    pub content_type: Option<String>,
    /// HTML title, when extracted.
    pub title: Option<String>,
}
impl Default for URLNode {
    fn default() -> Self {
        Self {
            url: String::new(),
            url_obj: Url::parse("about:blank").unwrap(),
            response_status: ResponseStatus::Unknown,
            depth: 0,
            content_length: None,
            content_type: None,
            title: None,
        }
    }
}
impl Hash for URLNode {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.url_obj.hash(state);
    }
}
impl PartialEq for URLNode {
    fn eq(&self, other: &Self) -> bool {
        self.url_obj == other.url_obj
    }
}
impl Eq for URLNode {}

impl URLNodeBuilder {
    fn validate(&self) -> Result<()> {
        if let Some(url) = &self.url
            && url.is_empty()
        {
            bail!("URL cannot be empty");
        }
        Ok(())
    }
    /// Build a [`URLNode`] and parse its URL string.
    pub fn build(&self) -> Result<URLNode> {
        let url = self.url.clone().unwrap_or_default();
        let url_obj = Url::parse(&url)?;
        self.validate()?;
        Ok(URLNode {
            url,
            url_obj,
            response_status: self
                .response_status
                .clone()
                .unwrap_or(ResponseStatus::Unknown),
            depth: self.depth.unwrap_or_default(),
            content_length: self.content_length.unwrap_or_default(),
            content_type: self.content_type.clone().unwrap_or_default(),
            title: self.title.clone().unwrap_or_default(),
        })
    }
}

#[derive(Builder)]
#[builder(pattern = "owned")]
#[allow(missing_docs)]
/// Extracts child URL nodes from text and HTML.
pub struct URLParser<H>
where
    H: Handler,
{
    #[builder(setter(strip_option))]
    handler: Option<H>,
}

impl<H: Handler> URLParser<H> {
    /// Extract URLs from `text` relative to `base_url`.
    pub fn extract_urls(&self, base_url: &URLNode, text: &str) -> Result<HashSet<URLNode>> {
        let mut found_urls: HashSet<URLNode> = HashSet::new();
        let mut hrefs: HashSet<String> = HashSet::new();
        // extract hrefs by regex
        if let Some(handler) = &self.handler {
            hrefs.extend(
                handler
                    .handle(text)?
                    .into_iter()
                    .map(|secret| secret.data)
                    .collect::<Vec<String>>(),
            );
        }
        // extract hrefs by html
        let doc = Html::parse_document(text);
        let a_sel = Selector::parse("a[href]")
            .map_err(|e| anyhow!("fail to parse selector a[href]: {e}"))?;
        let link_sel = Selector::parse("link[href]")
            .map_err(|e| anyhow!("fail to parse selector link[href]: {e}"))?;
        let script_sel = Selector::parse("script[src]")
            .map_err(|e| anyhow!("fail to parse selector script[src]: {e}"))?;

        for ele in doc.select(&a_sel) {
            if let Some(href) = ele.value().attr("href") {
                hrefs.insert(href.to_string());
            }
        }
        for ele in doc.select(&link_sel) {
            if let Some(href) = ele.value().attr("href") {
                hrefs.insert(href.to_string());
            }
        }
        for ele in doc.select(&script_sel) {
            if let Some(href) = ele.value().attr("src")
                && href.ends_with(".js")
            {
                hrefs.insert(href.to_string());
            }
        }
        for mut href in hrefs {
            href = href.trim_matches('"').trim_matches('\'').to_string();
            if is_malformed(&href) {
                continue;
            }
            let mut url = urlparse(&href);
            if is_static_resource(&url.path) {
                continue;
            }
            href = sanitize_url(&href);
            if !should_resolve(&href) {
                continue;
            }
            url = urlparse(href);
            if is_localhost(&url.netloc) {
                continue;
            }
            if !url.scheme.is_empty()
                && !url.netloc.is_empty()
                && ["http", "https"].contains(&url.scheme.as_str())
            {
                // a valid url
                let node = URLNodeBuilder::default()
                    .url(url.unparse())
                    .depth(base_url.depth + 1)
                    .build()?;
                found_urls.insert(node);
            } else {
                // invalid url, derive host and scheme from base_url
                let mut url_obj = base_url
                    .url_obj
                    .join(&url.path)
                    .unwrap_or(base_url.url_obj.clone())
                    .to_owned();
                url_obj.set_query(url.query.as_deref());
                url_obj.set_fragment(url.fragment.as_deref());
                let node = URLNodeBuilder::default()
                    .url(url_obj.to_string())
                    .depth(base_url.depth + 1)
                    .build()?;
                found_urls.insert(node);
            }
        }
        Ok(found_urls)
    }
}

fn is_static_resource(path: &str) -> bool {
    STATIC_EXTS.iter().any(|ext| path.ends_with(ext))
}
fn sanitize_url(url: &str) -> String {
    let url = url
        .replace(" ", "")
        .replace("\\/", "/")
        .replace("%3A", ":")
        .replace("%2F", "/");
    let url = url.trim();
    if WORDS.find(url).is_none() {
        return "".to_string();
    }
    if let Some(m) = IGNORED_URL.find(url)
        && !m.as_str().is_empty()
    {
        return "".to_string();
    }
    if url.starts_with("javascript") {
        return "".to_string();
    }
    url.to_owned()
}
fn is_localhost(netloc: &str) -> bool {
    netloc.starts_with("127.0.0.1") || netloc.starts_with("localhost")
}
/// Extract and normalize `<title>` text from an HTML response body.
pub fn response_title(response_str: &str) -> Result<String> {
    let doc = Html::parse_document(response_str);
    let title_sel =
        Selector::parse("title").map_err(|e| anyhow!("fail to parse title selector: {e}"))?;
    Ok(doc
        .select(&title_sel)
        .map(|ele| ele.text().collect())
        .map(|s: String| s.replace("\n", " ").replace("\r", " ").trim().to_string())
        .collect::<Vec<String>>()
        .join("|"))
}
// /// return (host, port, netloc)
// pub fn url_to_netloc(url: &Url) -> Option<String> {
//     let host = url.host_str()?.to_string();
//     url.port_or_known_default()
//         .map(|p| format!("{}:{}", host, p).to_string())
// }
fn should_resolve(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with("javascript:")
        && !s.starts_with("mailto:")
        && !s.starts_with("tel:")
}
fn is_malformed(href: &str) -> bool {
    let href = href.to_lowercase();
    if let Ok(href) = decode(&href) {
        href.contains("'")
            || href.contains('"')
            || href.contains("href=")
            || href.contains("src=")
            || href.contains("action=")
    } else {
        href.contains('"') || href.contains("'") || href.contains("%22") || href.contains("href%3d")
    }
}
