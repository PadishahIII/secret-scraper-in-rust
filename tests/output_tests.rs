use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    io::{self, Write},
    rc::Rc,
};

use secret_scraper::{
    handler::Secret,
    output::{Formatter, URLType, output_csv},
    urlparser::{ResponseStatus, URLNode, URLNodeBuilder},
};

fn node(url: &str, status: ResponseStatus) -> URLNode {
    URLNodeBuilder::default()
        .url(url.to_string())
        .response_status(status)
        .depth(0)
        .build()
        .expect("valid URL node")
}

fn detailed_node(
    url: &str,
    status: ResponseStatus,
    content_length: Option<u64>,
    content_type: Option<&'static str>,
    title: Option<&'static str>,
) -> URLNode {
    URLNodeBuilder::default()
        .url(url.to_string())
        .response_status(status)
        .depth(0)
        .content_length(content_length)
        .content_type(content_type.map(str::to_string))
        .title(title.map(str::to_string))
        .build()
        .expect("valid URL node")
}

fn strip_ansi(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for code_ch in chars.by_ref() {
                if code_ch == 'm' {
                    break;
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn domains(items: &[&str]) -> HashSet<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

fn children(items: Vec<URLNode>) -> HashSet<URLNode> {
    items.into_iter().collect()
}

#[derive(Clone, Default)]
struct SharedBuffer(Rc<RefCell<Vec<u8>>>);

impl SharedBuffer {
    fn into_string(&self) -> String {
        String::from_utf8(self.0.borrow().clone()).expect("utf8 csv")
    }
}

impl Write for SharedBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn format_url_per_domain_groups_by_root_domain_and_includes_base_url() {
    let formatter = Formatter::new(None);
    let domains = domains(&["example.com"]);

    let base = node("https://www.example.com", ResponseStatus::Valid(200));
    let child = node("https://api.example.com/users", ResponseStatus::Valid(200));
    let mut urls = HashMap::new();
    urls.insert(base, children(vec![child]));

    let output = strip_ansi(&formatter.format_url_per_domain(&domains, &urls, URLType::Url));

    assert!(output.contains("2 URL from example.com:\n"));
    assert!(output.contains("https://www.example.com [200]"));
    assert!(output.contains("https://api.example.com/users [200]"));
    assert!(!output.contains("Other"));
}

#[test]
fn format_url_per_domain_places_external_root_domains_in_other_last() {
    let formatter = Formatter::new(None);
    let domains = domains(&["example.com"]);

    let base = node("https://www.example.com", ResponseStatus::Valid(200));
    let first_party = node("https://cdn.example.com/app.js", ResponseStatus::Valid(200));
    let external = node("https://cdn.other.net/lib.js", ResponseStatus::Valid(200));
    let mut urls = HashMap::new();
    urls.insert(base, children(vec![first_party, external]));

    let output = strip_ansi(&formatter.format_url_per_domain(&domains, &urls, URLType::JS));

    let first_party_section = output
        .find("2 JS from example.com:")
        .expect("first-party section");
    let other_section = output.find("1 JS from Other:").expect("other section");

    assert!(first_party_section < other_section);
    assert!(output.contains("https://www.example.com [200]"));
    assert!(output.contains("https://cdn.example.com/app.js [200]"));
    assert!(output.contains("https://cdn.other.net/lib.js [200]"));
}

#[test]
fn format_url_per_domain_counts_only_filtered_urls() {
    let formatter = Formatter::new(None);
    let domains = domains(&["example.com"]);

    let base = node("https://example.com", ResponseStatus::Valid(200));
    let ok = node("https://example.com/ok", ResponseStatus::Valid(200));
    let missing = node("https://example.com/missing", ResponseStatus::Valid(404));
    let mut urls = HashMap::new();
    urls.insert(base, children(vec![ok, missing]));

    let output = strip_ansi(&formatter.format_url_per_domain(&domains, &urls, URLType::Url));

    assert!(output.contains("2 URL from example.com:"));
    assert!(output.contains("https://example.com [200]"));
    assert!(output.contains("https://example.com/ok [200]"));
    assert!(!output.contains("https://example.com/missing"));
}

#[test]
fn format_url_per_domain_filters_ignored_urls() {
    let formatter = Formatter::new(None);
    let domains = domains(&["example.com"]);

    let base = node("https://example.com", ResponseStatus::Valid(200));
    let ignored = node("https://example.com/image.png", ResponseStatus::Ignore);
    let mut urls = HashMap::new();
    urls.insert(base, children(vec![ignored]));

    let output = strip_ansi(&formatter.format_url_per_domain(&domains, &urls, URLType::Url));

    assert!(output.contains("1 URL from example.com:"));
    assert!(output.contains("https://example.com [200]"));
    assert!(!output.contains("https://example.com/image.png"));
}

#[test]
fn output_csv_writes_secret_and_hierarchy_urls() {
    let base = detailed_node(
        "https://example.com",
        ResponseStatus::Valid(200),
        Some(100),
        Some("text/html"),
        Some("Home"),
    );
    let child = detailed_node(
        "https://example.com/app.js",
        ResponseStatus::Valid(200),
        Some(42),
        Some("application/javascript"),
        None,
    );
    let secret_only = detailed_node(
        "https://api.example.com/config",
        ResponseStatus::Valid(403),
        None,
        Some("application/json"),
        Some("Config"),
    );
    let secrets = HashSet::from([Secret {
        secret_type: "API Key".to_string(),
        data: "secret-value".to_string(),
    }]);

    let children = children(vec![child]);
    let mut urls = HashMap::new();
    urls.insert(base, children);

    let mut url_secrets = HashMap::new();
    url_secrets.insert(secret_only, secrets);

    let output = SharedBuffer::default();
    let count = output_csv(Box::new(output.clone()), &urls, &url_secrets).expect("csv output");
    let csv = output.into_string();

    assert_eq!(count, 3);
    assert!(csv.contains("URL,Title,Response Code,Content Length,Content Type,Secrets"));
    assert!(csv.contains(
        "https://api.example.com/config,Config,403,0,application/json,API Key: secret-value"
    ));
    assert!(csv.contains("https://example.com,Home,200,100,text/html,"));
    assert!(csv.contains("https://example.com/app.js,,200,42,application/javascript,"));
}
