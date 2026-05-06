use std::{collections::HashSet, fmt::Display, hash::Hash};

use anyhow::{Result, anyhow, bail};
use derive_builder::Builder;
use lazy_static::lazy_static;
use regex::Regex;
use scraper::{Html, Selector};
use url::Url;

static STATIC_EXTS: &[&str] = &[
    ".png", ".jpg", ".jpeg", ".gif", ".css", ".ico", ".dtd", ".svg", ".scss", ".vue", ".ts",
];
lazy_static! {
static ref IGNORED_URL:Regex = Regex::new("\\<|\\>|\\{|\\}|\\[|\\]|\\||\\^|;|/node_modules/|www\\.w3\\.org|example\\.com|jquery[-\\.\\w]*?\\.js|\\.src|\\.replace|\\.url|\\.att|\\.href|location\\.href|javascript:|location:|application/x-www-form-urlencoded|\\.createObject|:location|\\.path|\\*#__PURE__\\*|\\*\\$0\\*|\\n").unwrap();
static ref WORDS: Regex = Regex::new("[a-zA-Z0-9]+").unwrap();
}

#[derive(Debug, Clone)]
pub enum ResponseStatus<'a> {
    Unknown,
    Valid(&'a str),
}
impl<'a> Display for ResponseStatus<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResponseStatus::Unknown => write!(f, "Unknown"),
            ResponseStatus::Valid(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Builder)]
#[builder(default, build_fn(skip))]
pub struct URLNode<'a> {
    pub url: String,
    #[builder(setter(skip))]
    pub url_obj: Url,
    #[builder(default=ResponseStatus::Unknown)]
    pub response_status: ResponseStatus<'a>,
    pub depth: u32,
    #[builder(setter(strip_option))]
    pub parent: Option<&'a URLNode<'a>>,
    pub content_length: Option<u32>,
    pub content_type: Option<&'a str>,
    pub title: Option<&'a str>,
}
impl<'a> Default for URLNode<'a> {
    fn default() -> Self {
        Self {
            url: String::new(),
            url_obj: Url::parse("about:blank").unwrap(),
            response_status: ResponseStatus::Unknown,
            depth: 0,
            parent: None,
            content_length: None,
            content_type: None,
            title: None,
        }
    }
}
impl<'a> Hash for URLNode<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.url_obj.hash(state);
    }
}
impl<'a> PartialEq for URLNode<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.url_obj == other.url_obj
    }
}
impl<'a> Eq for URLNode<'a> {}

impl<'a> URLNodeBuilder<'a> {
    fn validate(&self) -> Result<()> {
        if let Some(url) = &self.url
            && url.is_empty()
        {
            bail!("URL cannot be empty");
        }
        match self.parent.unwrap_or_default() {
            Some(parent) if self.depth.unwrap_or_default() <= parent.depth => {
                Err(anyhow!("Depth must be greater than parent's depth"))
            }
            _ => Ok(()),
        }
    }
    pub fn build(&self) -> Result<URLNode<'a>> {
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
            parent: self.parent.unwrap_or_default(),
            content_length: self.content_length.unwrap_or_default(),
            content_type: self.content_type.unwrap_or_default(),
            title: self.title.unwrap_or_default(),
        })
    }
}

pub struct URLParser {}
impl URLParser {
    pub fn extract_urls<'a>(
        &self,
        base_url: &'a URLNode,
        text: &str,
    ) -> Result<HashSet<URLNode<'a>>> {
        let mut found_urls: HashSet<URLNode> = HashSet::new();

        let doc = Html::parse_document(text);
        let a_sel = Selector::parse("a[href]").unwrap();
        let link_sel = Selector::parse("link[href]").unwrap();
        let script_sel = Selector::parse("script[src]").unwrap();

        let mut hrefs: HashSet<String> = HashSet::new();
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
                    {
                        // a full url
                        let node = URLNodeBuilder::default()
                            .url(url.to_string())
                            .parent(base_url)
                            .depth(base_url.depth + 1)
                            .build()?;
                        found_urls.insert(node);
                    } else {
                        // only a path on base_url
                        let mut url_obj = base_url.url_obj.clone();
                        url_obj.set_path(url.path());
                        url_obj.set_query(url.query());
                        url_obj.set_fragment(url.fragment());
                        let node = URLNodeBuilder::default()
                            .url(url_obj.to_string())
                            .parent(base_url)
                            .depth(base_url.depth + 1)
                            .build()?;
                        found_urls.insert(node);
                    }
                }
                Err(e) => {
                    tracing::debug!("fail to parse href {href}: {e}");
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
    if let None = WORDS.find(url) {
        return "".to_string();
    }
    if let Some(_) = IGNORED_URL.find(url) {
        return "".to_string();
    }
    if url.starts_with("javascript") {
        return "".to_string();
    }
    "".to_string()
}
fn is_localhost(url: &Url) -> bool {
    let u = url.host_str().unwrap_or_default();
    u.starts_with("127.0.0.1") || u.starts_with("localhost")
}
