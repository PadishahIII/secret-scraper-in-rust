use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc, Mutex, MutexGuard, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use secret_scraper::{
    cli::{Config, Rule},
    facade::{CrawlerFacade, ScanFacade, ScanResult},
    urlparser::ResponseStatus,
};

#[derive(Clone)]
struct ResponseSpec {
    status: u16,
    reason: &'static str,
    content_type: &'static str,
    body: String,
    location: Option<String>,
}

impl ResponseSpec {
    fn html(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            reason: "OK",
            content_type: "text/html",
            body: body.into(),
            location: None,
        }
    }

    fn json(body: impl Into<String>) -> Self {
        Self {
            status: 200,
            reason: "OK",
            content_type: "application/json",
            body: body.into(),
            location: None,
        }
    }

    fn binary() -> Self {
        Self {
            status: 200,
            reason: "OK",
            content_type: "image/png",
            body: "png-bytes".to_string(),
            location: None,
        }
    }

    fn redirect(location: impl Into<String>) -> Self {
        Self {
            status: 302,
            reason: "Found",
            content_type: "text/plain",
            body: String::new(),
            location: Some(location.into()),
        }
    }
}

#[derive(Default, Clone)]
struct RequestLog {
    paths: Arc<Mutex<Vec<String>>>,
    headers: Arc<Mutex<Vec<HashMap<String, String>>>>,
}

impl RequestLog {
    fn record(&self, path: String, headers: HashMap<String, String>) {
        self.paths.lock().expect("paths lock").push(path);
        self.headers.lock().expect("headers lock").push(headers);
    }

    fn count(&self, path: &str) -> usize {
        self.paths
            .lock()
            .expect("paths lock")
            .iter()
            .filter(|p| p.as_str() == path)
            .count()
    }

    fn total(&self) -> usize {
        self.paths.lock().expect("paths lock").len()
    }

    fn paths(&self) -> Vec<String> {
        self.paths.lock().expect("paths lock").clone()
    }

    fn has_header(&self, name: &str, value: &str) -> bool {
        self.headers
            .lock()
            .expect("headers lock")
            .iter()
            .any(|headers| headers.get(name).is_some_and(|v| v == value))
    }
}

struct TestServer {
    addr: SocketAddr,
    log: RequestLog,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl TestServer {
    fn start(routes: HashMap<String, ResponseSpec>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        listener
            .set_nonblocking(false)
            .expect("set listener blocking");
        let addr = listener.local_addr().expect("local addr");
        let routes = Arc::new(routes);
        let log = RequestLog::default();
        let server_log = log.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let server_shutdown = shutdown.clone();
        let (ready_tx, ready_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let _ = ready_tx.send(());
            while !server_shutdown.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let routes = routes.clone();
                        let log = server_log.clone();
                        thread::spawn(move || handle_connection(stream, &routes, &log));
                    }
                    Err(e) if is_transient_accept_error(&e) => continue,
                    Err(_) if server_shutdown.load(Ordering::Relaxed) => break,
                    Err(_) => continue,
                }
            }
        });
        ready_rx.recv().expect("test server ready");

        Self {
            addr,
            log,
            shutdown,
            handle: Some(handle),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }
}

fn is_transient_accept_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::Interrupted
            | std::io::ErrorKind::WouldBlock
    )
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.addr);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn handle_connection(
    mut stream: TcpStream,
    routes: &HashMap<String, ResponseSpec>,
    log: &RequestLog,
) {
    let mut buffer = [0_u8; 8192];
    let bytes = match stream.read(&mut buffer) {
        Ok(0) | Err(_) => return,
        Ok(bytes) => bytes,
    };
    let request = String::from_utf8_lossy(&buffer[..bytes]);
    let mut lines = request.lines();
    let Some(request_line) = lines.next() else {
        return;
    };
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_string();
    let headers = lines
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect::<HashMap<_, _>>();
    log.record(path.clone(), headers);

    let response = routes.get(&path).cloned().unwrap_or_else(|| ResponseSpec {
        status: 404,
        reason: "Not Found",
        content_type: "text/plain",
        body: "not found".to_string(),
        location: None,
    });
    let mut raw = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status,
        response.reason,
        response.content_type,
        response.body.len()
    );
    if let Some(location) = response.location {
        raw.push_str(&format!("Location: {location}\r\n"));
    }
    raw.push_str("\r\n");
    raw.push_str(&response.body);
    let _ = stream.write_all(raw.as_bytes());
    let _ = stream.flush();
}

fn crawler_config(url: Option<String>) -> Config {
    Config {
        url,
        max_page: Some(20),
        max_depth: Some(1),
        timeout: Duration::from_secs(2),
        min_request_interval: Duration::ZERO,
        max_concurrency_per_domain: 8,
        url_find_rules: vec![
            Rule::new("path".to_string(), r#""(/[A-Za-z0-9_.?=&%-]+)""#).expect("url rule"),
        ],
        js_find_rules: vec![
            Rule::new("js".to_string(), r#""(/[A-Za-z0-9_.?=&%-]+\.js)""#).expect("js rule"),
        ],
        custom_rules: vec![
            Rule::new("secret".to_string(), r"SECRET_[A-Z0-9]+").expect("secret rule"),
        ],
        ..Config::default()
    }
}

fn run_facade(config: Config) {
    let _ = run_facade_and_return(config);
}

fn run_facade_and_return(config: Config) -> ScanResult {
    assert!(!config.url_find_rules.is_empty(), "url rules should be set");
    assert!(
        !config.custom_rules.is_empty(),
        "custom rules should be set"
    );
    thread::spawn(move || -> ScanResult {
        Box::new(CrawlerFacade::new(config).expect("crawler facade"))
            .scan()
            .expect("scan crawler facade")
    })
    .join()
    .expect("facade thread")
}

fn facade_test_guard() -> MutexGuard<'static, ()> {
    static FACADE_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    FACADE_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn temp_url_file(contents: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    path.push(format!("secret-scraper-crawler-facade-{nanos}.txt"));
    fs::write(&path, contents).expect("write url file");
    path
}

#[test]
fn crawler_facade_integration_scenarios() {
    scenario_crawls_seed_and_html_children();
    scenario_reads_multiple_seeds_from_url_file();
    scenario_applies_max_depth_to_discovered_links();
    scenario_validates_found_but_not_crawled_frontier_urls();
    scenario_skips_dangerous_paths();
    scenario_applies_disallow_domain_filter_to_seed();
    scenario_sends_custom_headers();
    scenario_ignores_non_processable_content_without_crawling_body_links();
    scenario_processes_json_with_regex_discovered_links();
    scenario_respects_redirect_policy();
    scenario_validate_marks_dangerous_frontier_urls_ignored();
    scenario_validate_marks_failed_frontier_urls_failed();
    scenario_max_page_counts_ignored_responses();
}

fn scenario_crawls_seed_and_html_children() {
    let _guard = facade_test_guard();
    let server = TestServer::start(HashMap::from([
        (
            "/index".to_string(),
            ResponseSpec::html(r#"<a href="/child">child</a>"#),
        ),
        ("/child".to_string(), ResponseSpec::html("child")),
    ]));

    run_facade(crawler_config(Some(server.url("/index"))));

    assert_eq!(server.log.count("/index"), 1, "{:?}", server.log.paths());
    assert_eq!(server.log.count("/child"), 1);
    assert!(server.log.has_header("accept", "*/*"));
    assert!(
        server
            .log
            .headers
            .lock()
            .expect("headers lock")
            .iter()
            .any(|headers| headers.contains_key("user-agent"))
    );
}

fn scenario_reads_multiple_seeds_from_url_file() {
    let _guard = facade_test_guard();
    let first_server = TestServer::start(HashMap::from([(
        "/seed-one".to_string(),
        ResponseSpec::html("one"),
    )]));
    let second_server = TestServer::start(HashMap::from([(
        "/two".to_string(),
        ResponseSpec::html("two"),
    )]));
    let path = temp_url_file(&format!(
        "\n{}\n{}\n\n",
        first_server.url("/seed-one"),
        second_server.url("/two")
    ));
    let mut config = crawler_config(None);
    config.url_file = Some(path.clone());

    run_facade(config);

    assert_eq!(
        first_server.log.count("/seed-one"),
        1,
        "{:?}",
        first_server.log.paths()
    );
    assert_eq!(
        second_server.log.count("/two"),
        1,
        "{:?}",
        second_server.log.paths()
    );
    let _ = fs::remove_file(path);
}

fn scenario_applies_max_depth_to_discovered_links() {
    let _guard = facade_test_guard();
    let server = TestServer::start(HashMap::from([
        (
            "/".to_string(),
            ResponseSpec::html(r#"<a href="/level-one">level one</a>"#),
        ),
        (
            "/level-one".to_string(),
            ResponseSpec::html(r#"<a href="/level-two">level two</a>"#),
        ),
        ("/level-two".to_string(), ResponseSpec::html("level two")),
    ]));
    let mut config = crawler_config(Some(server.url("/")));
    config.max_depth = Some(0);

    run_facade(config);

    assert_eq!(server.log.count("/"), 1, "{:?}", server.log.paths());
    assert_eq!(server.log.count("/level-one"), 0);
    assert_eq!(server.log.count("/level-two"), 0);
}

fn scenario_validates_found_but_not_crawled_frontier_urls() {
    let _guard = facade_test_guard();
    let server = TestServer::start(HashMap::from([
        (
            "/root".to_string(),
            ResponseSpec::html(r#"<a href="/validate-me">validate me</a>"#),
        ),
        ("/validate-me".to_string(), ResponseSpec::html("validated")),
    ]));
    let mut config = crawler_config(Some(server.url("/root")));
    config.max_depth = Some(0);
    config.validate = true;

    run_facade(config);

    assert_eq!(server.log.count("/root"), 1);
    assert_eq!(server.log.count("/validate-me"), 1);
}

fn scenario_skips_dangerous_paths() {
    let _guard = facade_test_guard();
    let server = TestServer::start(HashMap::from([
        (
            "/".to_string(),
            ResponseSpec::html(r#"<a href="/logout">logout</a><a href="/safe">safe</a>"#),
        ),
        ("/logout".to_string(), ResponseSpec::html("logout")),
        ("/safe".to_string(), ResponseSpec::html("safe")),
    ]));
    let mut config = crawler_config(Some(server.url("/")));
    config.dangerous_paths = Some(vec!["logout".to_string()]);

    run_facade(config);

    assert_eq!(server.log.count("/"), 1);
    assert_eq!(server.log.count("/logout"), 0);
    assert_eq!(server.log.count("/safe"), 1);
}

fn scenario_applies_disallow_domain_filter_to_seed() {
    let _guard = facade_test_guard();
    let server = TestServer::start(HashMap::from([(
        "/".to_string(),
        ResponseSpec::html("blocked"),
    )]));
    let mut config = crawler_config(Some(server.url("/")));
    config.disallow_domains = Some(vec!["*".to_string()]);

    run_facade(config);

    assert_eq!(server.log.total(), 0);
}

fn scenario_sends_custom_headers() {
    let _guard = facade_test_guard();
    let server = TestServer::start(HashMap::from([(
        "/".to_string(),
        ResponseSpec::html("headers"),
    )]));
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-test-header"),
        HeaderValue::from_static("crawler-facade"),
    );
    let mut config = crawler_config(Some(server.url("/")));
    config.custom_headers = Some(headers);

    run_facade(config);

    assert!(
        server.log.has_header("x-test-header", "crawler-facade"),
        "{:?}",
        server.log.headers.lock().expect("headers lock")
    );
}

fn scenario_ignores_non_processable_content_without_crawling_body_links() {
    let _guard = facade_test_guard();
    let server = TestServer::start(HashMap::from([
        ("/binary".to_string(), ResponseSpec::binary()),
        ("/hidden".to_string(), ResponseSpec::html("hidden")),
    ]));
    let mut config = crawler_config(Some(server.url("/binary")));
    config.url_find_rules = vec![Rule::new("hidden".to_string(), r"/hidden").expect("rule")];

    run_facade(config);

    assert_eq!(server.log.count("/binary"), 1, "{:?}", server.log.paths());
    assert_eq!(server.log.count("/hidden"), 0);
}

fn scenario_processes_json_with_regex_discovered_links() {
    let _guard = facade_test_guard();
    let server = TestServer::start(HashMap::from([
        (
            "/json".to_string(),
            ResponseSpec::json(r#"{"next":"/from-json"}"#),
        ),
        ("/from-json".to_string(), ResponseSpec::html("from json")),
    ]));
    let mut config = crawler_config(Some(server.url("/json")));
    config.url_find_rules = vec![Rule::new("json_path".to_string(), r"/from-json").expect("rule")];

    run_facade(config);

    assert_eq!(server.log.count("/json"), 1, "{:?}", server.log.paths());
    assert_eq!(
        server.log.count("/from-json"),
        1,
        "{:?}",
        server.log.paths()
    );
}

fn scenario_respects_redirect_policy() {
    let _guard = facade_test_guard();
    let server = TestServer::start(HashMap::from([
        ("/redirect".to_string(), ResponseSpec::redirect("/final")),
        ("/final".to_string(), ResponseSpec::html("final")),
    ]));
    let mut config = crawler_config(Some(server.url("/redirect")));
    config.follow_redirect = true;

    run_facade(config);

    assert_eq!(server.log.count("/redirect"), 1);
    assert_eq!(server.log.count("/final"), 1);
}

fn scenario_validate_marks_dangerous_frontier_urls_ignored() {
    let _guard = facade_test_guard();
    let server = TestServer::start(HashMap::from([
        (
            "/root".to_string(),
            ResponseSpec::html(r#"<a href="/logout">logout</a>"#),
        ),
        ("/logout".to_string(), ResponseSpec::html("logout")),
    ]));
    let mut config = crawler_config(Some(server.url("/root")));
    config.max_depth = Some(0);
    config.validate = true;
    config.dangerous_paths = Some(vec!["logout".to_string()]);

    let result = run_facade_and_return(config);

    assert_eq!(server.log.count("/root"), 1, "{:?}", server.log.paths());
    assert_eq!(server.log.count("/logout"), 0);
    let ScanResult::CrawlResult(result) = result else {
        panic!("expected crawl result");
    };
    let logout = result
        .urls
        .values()
        .flatten()
        .find(|node| node.url.ends_with("/logout"))
        .expect("logout frontier node");
    assert!(matches!(logout.response_status, ResponseStatus::Ignore));
}

fn scenario_validate_marks_failed_frontier_urls_failed() {
    let _guard = facade_test_guard();
    let server = TestServer::start(HashMap::from([(
        "/root".to_string(),
        ResponseSpec::html(r#"<a href="/missing">missing</a>"#),
    )]));
    let mut config = crawler_config(Some(server.url("/root")));
    config.max_depth = Some(0);
    config.validate = true;

    let result = run_facade_and_return(config);

    assert_eq!(server.log.count("/root"), 1, "{:?}", server.log.paths());
    assert_eq!(server.log.count("/missing"), 1);
    let ScanResult::CrawlResult(result) = result else {
        panic!("expected crawl result");
    };
    let missing = result
        .urls
        .values()
        .flatten()
        .find(|node| node.url.ends_with("/missing"))
        .expect("missing frontier node");
    assert!(matches!(
        missing.response_status,
        ResponseStatus::Valid(404)
    ));
}

fn scenario_max_page_counts_ignored_responses() {
    let _guard = facade_test_guard();
    let server = TestServer::start(HashMap::from([
        (
            "/root".to_string(),
            ResponseSpec::html(r#"<a href=/binary>binary</a>"#),
        ),
        ("/binary".to_string(), ResponseSpec::binary()),
        (
            "/from-binary".to_string(),
            ResponseSpec::html("from binary"),
        ),
    ]));
    let mut config = crawler_config(Some(server.url("/root")));
    config.max_page = Some(2);

    run_facade(config);

    assert_eq!(server.log.count("/root"), 1, "{:?}", server.log.paths());
    assert_eq!(server.log.count("/binary"), 1, "{:?}", server.log.paths());
    assert_eq!(
        server.log.count("/from-binary"),
        0,
        "{:?}",
        server.log.paths()
    );
}
