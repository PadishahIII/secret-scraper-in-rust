use std::{collections::HashSet, fmt::Display, hash::Hash};

use anyhow::{Result, anyhow, bail};
use derive_builder::Builder;
use lazy_static::lazy_static;
use regex::Regex;
use scraper::{Html, Selector};
use url::Url;

use crate::handler::Handler;

static STATIC_EXTS: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".gif", ".css", ".ico", ".dtd", ".svg", ".scss", ".vue", ".ts",
];
lazy_static! {
static ref IGNORED_URL:Regex = Regex::new("\\<|\\>|\\{|\\}|\\[|\\]|\\||\\^|;|/node_modules/|www\\.w3\\.org|example\\.com|jquery[-\\.\\w]*?\\.js|\\.src|\\.replace|\\.url|\\.att|\\.href|location\\.href|javascript:|location:|application/x-www-form-urlencoded|\\.createObject|:location|\\.path|\\*#__PURE__\\*|\\*\\$0\\*|\\n").unwrap();
static ref WORDS: Regex = Regex::new("[a-zA-Z0-9]+").unwrap();
}

#[derive(Debug, Clone)]
pub enum ResponseStatus {
    /// the url is not requested yet
    Unknown,
    Valid(u16),
    /// invalid response status
    Failed(String),
}
impl Display for ResponseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResponseStatus::Unknown => write!(f, "Unknown"),
            ResponseStatus::Valid(s) => write!(f, "{s}"),
            ResponseStatus::Failed(e) => write!(f, "Failed: {e}"),
        }
    }
}

#[derive(Debug, Builder, Clone)]
#[builder(default, build_fn(skip))]
pub struct URLNode {
    pub url: String,
    #[builder(setter(skip))]
    pub url_obj: Url,
    #[builder(default=ResponseStatus::Unknown)]
    pub response_status: ResponseStatus,
    pub depth: u32,
    pub content_length: Option<u64>,
    pub content_type: Option<String>,
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
    pub fn build(&self) -> Result<URLNode> {
        let url = self.url.clone().unwrap_or_default();
        let url_obj = Url::parse(&url)?;
        self.validate()?;
        Ok(URLNode {
            url,
            url_obj,
            response_status: self.response_status.unwrap_or(ResponseStatus::Unknown),
            depth: self.depth.unwrap_or_default(),
            content_length: self.content_length.unwrap_or_default(),
            content_type: self.content_type.clone().unwrap_or_default(),
            title: self.title.clone().unwrap_or_default(),
        })
    }
}

#[derive(Builder)]
pub struct URLParser<H>
where
    H: Handler,
{
    handler: Option<H>,
}

impl<H: Handler> URLParser<H> {
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
        for href in hrefs {
            match Url::parse(&href) {
                Ok(url) => {
                    if is_static_resource(url.path()) {
                        continue;
                    }
                    if sanitize_url(&href).is_empty() || is_localhost(&url) {
                        continue;
                    }
                    if !url.scheme().is_empty()
                        && url.host().is_some()
                        && !url.host_str().unwrap_or_default().is_empty()
                        && ["http", "https"].contains(&url.scheme())
                    {
                        // a valid url
                        let node = URLNodeBuilder::default()
                            .url(url.to_string())
                            .depth(base_url.depth + 1)
                            .build()?;
                        found_urls.insert(node);
                    } else {
                        // invalid url, derive host and scheme from base_url
                        let url_obj = base_url.url_obj.clone();
                        let mut url_obj = url_obj.join(url.path()).unwrap_or(url_obj).to_owned();
                        url_obj.set_query(url.query());
                        url_obj.set_fragment(url.fragment());
                        let node = URLNodeBuilder::default()
                            .url(url_obj.to_string())
                            .depth(base_url.depth + 1)
                            .build()?;
                        found_urls.insert(node);
                    }
                }
                Err(_) => {
                    // assume [`href`] is always a path here
                    let url_obj = base_url.url_obj.clone();
                    let url_obj = url_obj.join(&href).unwrap_or(url_obj).to_owned();
                    let node = URLNodeBuilder::default()
                        .url(url_obj.to_string())
                        .depth(base_url.depth + 1)
                        .build()?;
                    found_urls.insert(node);
                }
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
fn is_localhost(url: &Url) -> bool {
    let u = url.host_str().unwrap_or_default();
    u.starts_with("127.0.0.1") || u.starts_with("localhost")
}
fn response_title(response_str: &str) -> Result<String> {
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
/// Get the directory part of a path, including the trailing slash. If there is no slash, return an empty string.
fn dir_of(path: String) -> Option<String> {
    path.rfind('/').map(|i| path[..=i].into())
}
