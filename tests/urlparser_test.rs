use std::collections::BTreeSet;

use anyhow::Result;
use secret_scraper::{
    handler::{Handler, Secret},
    urlparser::{URLNode, URLNodeBuilder, URLParserBuilder},
};

#[derive(Default, Clone)]
struct EmptyHandler;

impl Handler for EmptyHandler {
    fn handle(&self, _text: &str) -> Result<Vec<Secret>> {
        Ok(Vec::new())
    }
}

#[derive(Clone)]
struct StaticHandler {
    secrets: Vec<&'static str>,
}

impl Handler for StaticHandler {
    fn handle(&self, _text: &str) -> Result<Vec<Secret>> {
        Ok(self
            .secrets
            .iter()
            .map(|data| Secret {
                secret_type: "url".to_string(),
                data: (*data).to_string(),
            })
            .collect())
    }
}

fn root_node(url: &str) -> URLNode<'static> {
    URLNodeBuilder::default()
        .url(url.to_string())
        .depth(0)
        .build()
        .expect("valid root URL")
}

fn parser_without_regex() -> secret_scraper::urlparser::URLParser<EmptyHandler> {
    URLParserBuilder::default()
        .handler(Some(EmptyHandler))
        .build()
        .expect("valid parser")
}

fn parser_with_regex(
    secrets: Vec<&'static str>,
) -> secret_scraper::urlparser::URLParser<StaticHandler> {
    URLParserBuilder::default()
        .handler(Some(StaticHandler { secrets }))
        .build()
        .expect("valid parser")
}

fn extract_urls<'a>(
    parser: &secret_scraper::urlparser::URLParser<impl Handler>,
    base_url: &'a URLNode<'a>,
    html: &str,
) -> Vec<String> {
    parser
        .extract_urls(base_url, html)
        .expect("URL extraction succeeds")
        .into_iter()
        .map(|node| node.url)
        .collect::<Vec<_>>()
}

fn assert_urls_eq(actual: Vec<String>, expected: &[&str]) {
    let actual = actual.into_iter().collect::<BTreeSet<_>>();
    let expected = expected
        .iter()
        .map(|url| (*url).to_string())
        .collect::<BTreeSet<_>>();

    assert_eq!(actual, expected);
}

#[test]
fn extracts_absolute_urls_from_html_links_and_js_scripts() {
    let base = root_node("https://random.com/base/page.html");
    let parser = parser_without_regex();
    let html = r#"
        <a href="https://other.example/path">other</a>
        <link href="https://random.com/app">
        <script src="https://cdn.example/app.js"></script>
        <script src="https://cdn.example/app.css"></script>
    "#;

    let urls = extract_urls(&parser, &base, html);

    assert_urls_eq(
        urls,
        &[
            "https://cdn.example/app.js",
            "https://random.com/app",
            "https://other.example/path",
        ],
    );
}

#[test]
fn resolves_root_relative_urls_against_base_origin() {
    let base = root_node("https://random.com/base/page.html");
    let parser = parser_without_regex();
    let html = r#"<a href="/login?next=%2Fdashboard#top">login</a>"#;

    let urls = extract_urls(&parser, &base, html);

    assert_urls_eq(urls, &["https://random.com/login?next=%2Fdashboard#top"]);
}

#[test]
fn resolves_path_relative_urls_against_base_directory() {
    let base = root_node("https://random.com/base/page.html");
    let parser = parser_without_regex();
    let html = r#"<a href="assets/app">app</a>"#;

    let urls = extract_urls(&parser, &base, html);

    assert_urls_eq(urls, &["https://random.com/base/assets/app"]);
}

#[test]
fn resolves_parent_directory_segments_against_base_directory() {
    let base = root_node("https://random.com/base/nested/page.html");
    let parser = parser_without_regex();
    let html = r#"<a href="../api/users">users</a>"#;

    let urls = extract_urls(&parser, &base, html);

    assert_urls_eq(urls, &["https://random.com/base/api/users"]);
}

#[test]
fn merges_regex_handler_urls_with_html_urls_and_deduplicates_by_url() {
    let base = root_node("https://random.com/base/page.html");
    let parser = parser_with_regex(vec![
        "https://api.random.com/v1/users",
        "https://api.random.com/v1/users",
    ]);
    let html = r#"<a href="https://random.com/docs">docs</a>"#;

    let urls = extract_urls(&parser, &base, html);

    assert_urls_eq(
        urls,
        &["https://api.random.com/v1/users", "https://random.com/docs"],
    );
}

#[test]
fn filters_static_resources_localhost_and_dirty_urls() {
    let base = root_node("https://random.com/base/page.html");
    let parser = parser_without_regex();
    let html = r#"
        <a href="https://random.com/image.png">image</a>
        <a href="https://localhost/admin">local</a>
        <a href="javascript:alert(1)">javascript</a>
        <a href="https://random.com/api">api</a>
    "#;

    let urls = extract_urls(&parser, &base, html);

    assert_urls_eq(urls, &["https://random.com/api"]);
}

#[test]
fn urlnode_equality_and_hash_use_parsed_url_only() {
    let first = URLNodeBuilder::default()
        .url("https://random.com/path".to_string())
        .depth(0)
        .build()
        .expect("valid URL node");
    let second = URLNodeBuilder::default()
        .url("https://random.com/path".to_string())
        .depth(5)
        .build()
        .expect("valid URL node");

    assert_eq!(first, second);
}

#[test]
fn urlnode_builder_rejects_empty_url() {
    let result = URLNodeBuilder::default().url(String::new()).build();

    assert!(result.is_err());
}

#[test]
fn urlnode_builder_requires_child_depth_to_exceed_parent_depth() {
    let parent = root_node("https://random.com/");

    let result = URLNodeBuilder::default()
        .url("https://random.com/child".to_string())
        .parent(&parent)
        .depth(parent.depth)
        .build();

    assert!(result.is_err());
}
