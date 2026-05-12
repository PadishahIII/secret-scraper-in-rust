//! # SecretScraper
//!
//! Rust library for crawling web targets, discovering URLs and JavaScript links,
//! and detecting secrets (API keys, credentials, internal IPs, PII, and more)
//! with configurable regular-expression rules. Also scans local files and
//! directories recursively.
//!
//! ## Quick Start
//!
//! Crawl a website with the built-in detection rules:
//!
//! ```rust,no_run
//! use secret_scraper::{
//!     cli::{Config, Mode},
//!     facade::{CrawlerFacade, ScanFacade, ScanResult},
//! };
//!
//! let mut config = Config::default_with_rules();
//! config.url = Some("https://example.com".to_string());
//! config.mode = Mode::Thorough;
//! config.detail = true;
//! config.outfile = Some("crawl.csv".into());
//!
//! match Box::new(CrawlerFacade::new(config).unwrap()).scan().unwrap() {
//!     ScanResult::CrawlResult(result) => {
//!         println!(
//!             "{} domains, {} URL groups, {} secret-bearing URLs",
//!             result.hosts.len(),
//!             result.urls.len(),
//!             result.secrets.len()
//!         );
//!     }
//!     ScanResult::LocalScanResult(_) => unreachable!(),
//! }
//! ```
//!
//! Scan a local directory recursively:
//!
//! ```rust,no_run
//! use secret_scraper::{
//!     cli::Config,
//!     facade::{FileScannerFacade, ScanFacade, ScanResult},
//! };
//!
//! let mut config = Config::default_with_rules();
//! config.local = Some("./samples".into());
//! config.outfile = Some("local-scan.yml".into());
//!
//! match Box::new(FileScannerFacade::new(config).unwrap()).scan().unwrap() {
//!     ScanResult::LocalScanResult(result) => {
//!         println!("{} files scanned", result.len());
//!         for (path, secrets) in &result {
//!             println!("{}: {} secrets", path.display(), secrets.len());
//!         }
//!     }
//!     ScanResult::CrawlResult(_) => unreachable!(),
//! }
//! ```
//!
//! ## Features
//!
//! - **Web crawling** — crawl seed URLs with configurable depth, following HTML
//!   links, JavaScript sources, and regex-discovered URLs.
//! - **Local file scanning** — scan a single file or walk a directory tree
//!   recursively for secrets.
//! - **Built-in secret rules** — detects Swagger docs, ID cards, phone numbers,
//!   email addresses, internal IPs, cloud keys, Shiro keys, API keys, and more.
//! - **Custom rules** — add your own regex patterns for URL discovery, JavaScript
//!   link extraction, and secret detection.
//! - **Domain filtering** — allow-list or block-list domains with wildcard
//!   patterns (`*.example.com`).
//! - **Rate limiting** — per-domain concurrency caps and minimum request intervals.
//! - **Proxy support** — HTTP and SOCKS5 proxies.
//! - **Status filtering** — filter displayed results by HTTP status codes or ranges.
//! - **Validation mode** — verify discovered link statuses without crawling them.
//! - **Output formats** — crawl results as CSV, local scan results as YAML.
//!
//! ## Configuration
//!
//! Build a [`Config`](cli::Config) by starting from a default, then setting
//! fields directly. The layering order used by the CLI (defaults → YAML → CLI
//! flags) is available programmatically via
//! [`apply_file_layer`](cli::Config::apply_file_layer) and
//! [`apply_cli_layer`](cli::Config::apply_cli_layer), but for library usage you
//! typically set fields directly on the struct.
//!
//! Two constructors are available:
//!
//! | Method | Description |
//! |---|---|
//! | [`Config::default()`](cli::Config::default) | Empty rule lists — add your own rules. |
//! | [`Config::default_with_rules()`](cli::Config::default_with_rules) | Pre-populated with 5 URL-find, 3 JS-find, and 10 secret-detection rules. |
//!
//! Key configuration fields on [`Config`](cli::Config):
//!
//! | Field | Type | Description |
//! |---|---|---|
//! | `url` | `Option<String>` | Single seed URL for crawling. |
//! | `url_file` | `Option<PathBuf>` | Newline-delimited file of seed URLs. |
//! | `local` | `Option<PathBuf>` | File or directory for local scanning. |
//! | `mode` | [`Mode`](cli::Mode) | `Normal` (depth 1) or `Thorough` (depth 2). |
//! | `max_depth` | `Option<u32>` | Override crawl depth; 0 = seed URLs only. |
//! | `max_page` | `Option<u32>` | Maximum pages to crawl (default 1000). |
//! | `detail` | `bool` | Show per-URL hierarchy in output. |
//! | `validate` | `bool` | Validate discovered link statuses. |
//! | `follow_redirect` | `bool` | Follow HTTP redirects. |
//! | `hide_regex` | `bool` | Suppress secret output. |
//! | `outfile` | `Option<PathBuf>` | Write results to file (CSV for crawl, YAML for scan). |
//! | `timeout` | `Duration` | Request timeout (default 30s). |
//! | `proxy` | `Option<String>` | Proxy URL (`http://host:port` or `socks5://host:port`). |
//! | `user_agent` | `Option<String>` | Override User-Agent header. |
//! | `cookie` | `Option<String>` | Set Cookie header. |
//! | `allow_domains` | `Option<Vec<String>>` | Domain allow-list with wildcards. |
//! | `disallow_domains` | `Option<Vec<String>>` | Domain block-list with wildcards. |
//! | `max_concurrency_per_domain` | `usize` | Concurrent request cap per domain (default 50). |
//! | `min_request_interval` | `Duration` | Minimum seconds between requests to same domain (default 200ms). |
//! | `dangerous_paths` | `Option<Vec<String>>` | Path fragments to avoid requesting (e.g. `logout`, `delete`). |
//! | `url_find_rules` | `Vec<Rule>` | Regex rules for discovering URLs in response text. |
//! | `js_find_rules` | `Vec<Rule>` | Regex rules for discovering JavaScript URLs. |
//! | `custom_rules` | `Vec<Rule>` | Regex rules for secret detection. |
//! | `custom_headers` | `Option<HeaderMap>` | Extra HTTP headers sent with requests. |
//! | `status_filter` | `Option<StatusRangeRule>` | Filter output by response status. |
//!
//! ## Custom Rules
//!
//! Use [`Rule::new`](cli::Rule::new) to compile a named regex:
//!
//! ```rust
//! use secret_scraper::cli::{Config, Rule};
//!
//! let mut config = Config::default();
//! config.url_find_rules.push(
//!     Rule::new_with_group("api_path".into(), r#""(/api/v[0-9]+/[^"]+)""#, true).unwrap()
//! );
//! config.custom_rules.push(
//!     Rule::new("Custom Token".into(), r"TOKEN_[A-Z0-9]{16}").unwrap()
//! );
//! ```
//!
//! `Rule::new` emits the full regex match. Use
//! [`Rule::new_with_group`](cli::Rule::new_with_group) when capture groups
//! should be emitted instead, which is usually what URL-discovery rules need.
//!
//! Rules added via `Config::default()` start empty. When using
//! `Config::default_with_rules()`, your custom rules are appended to the
//! built-in lists.
//!
//! ## Result Handling
//!
//! The high-level API uses [`ScanFacade::scan`](facade::ScanFacade::scan), which
//! returns [`ScanStdResult`](facade::ScanStdResult) — an alias for
//! `Result<ScanResult, SecretScraperError>`.
//!
//! ```rust,no_run
//! use secret_scraper::{
//!     cli::Config,
//!     error::{Result as SsResult, SecretScraperError},
//!     facade::{FileScannerFacade, ScanFacade, ScanResult},
//! };
//!
//! fn try_scan() -> SsResult<()> {
//!     let mut config = Config::default_with_rules();
//!     config.local = Some("./src".into());
//!
//!     match Box::new(FileScannerFacade::new(config)?).scan() {
//!         Ok(ScanResult::LocalScanResult(files)) => {
//!             for (path, secrets) in &files {
//!                 for s in secrets {
//!                     println!("{}: [{}] {}", path.display(), s.secret_type, s.data);
//!                 }
//!             }
//!         }
//!         Ok(ScanResult::CrawlResult(_)) => unreachable!(),
//!         Err(SecretScraperError::Scanner(msg)) => eprintln!("scan failed: {msg}"),
//!         Err(e) => eprintln!("error: {e}"),
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ## Advanced: Crawl with Full Options
//!
//! ```rust,no_run
//! use std::time::Duration;
//! use secret_scraper::{
//!     cli::{Config, Mode, Rule},
//!     facade::{CrawlerFacade, ScanFacade, ScanResult},
//! };
//!
//! let mut config = Config::default_with_rules();
//! config.url = Some("https://example.com".to_string());
//! config.mode = Mode::Thorough;
//! config.max_depth = Some(3);
//! config.max_page = Some(500);
//! config.max_concurrency_per_domain = 10;
//! config.min_request_interval = Duration::from_millis(500);
//! config.timeout = Duration::from_secs(15);
//! config.follow_redirect = true;
//! config.validate = true;
//! config.detail = true;
//! config.user_agent = Some("SecretScraper/0.1".into());
//! config.proxy = Some("http://127.0.0.1:8080".into());
//! config.allow_domains = Some(vec!["*.example.com".into()]);
//! config.dangerous_paths = Some(vec!["logout".into(), "delete".into()]);
//! config.outfile = Some("crawl.csv".into());
//! config.custom_rules.push(
//!     Rule::new("JWT".into(), r"eyJ[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+").unwrap()
//! );
//!
//! match Box::new(CrawlerFacade::new(config).unwrap()).scan().unwrap() {
//!     ScanResult::CrawlResult(result) => {
//!         println!(
//!             "Done: {} domains, {} URLs, {} JS files, {} secrets",
//!             result.hosts.len(),
//!             result.urls.len(),
//!             result.js.len(),
//!             result.secrets.len(),
//!         );
//!     }
//!     ScanResult::LocalScanResult(_) => unreachable!(),
//! }
//! ```
//!
//! ## Module Overview
//!
//! | Module | Purpose |
//! |---|---|
//! | [`cli`] | Configuration types: [`Config`](cli::Config), [`Mode`](cli::Mode), [`Rule`](cli::Rule). |
//! | [`facade`] | High-level entry points: [`CrawlerFacade`](facade::CrawlerFacade), [`FileScannerFacade`](facade::FileScannerFacade). |
//! | [`error`] | Error types: [`SecretScraperError`](error::SecretScraperError) and the [`Result`](error::Result) alias. |
//! | [`handler`] | Secret detection: [`RegexHandler`](handler::RegexHandler), [`Secret`](handler::Secret). |
//! | [`urlparser`] | URL representation: [`URLNode`](urlparser::URLNode), [`ResponseStatus`](urlparser::ResponseStatus). |
//! | [`filter`] | Domain allow-list / block-list filter chain. |
//! | [`output`] | Human-readable and CSV output formatting. |
//! | [`rate_limiter`] | Per-domain request rate limiting. |
//! | [`scanner`] | Local file traversal and scanning engine. |
//! | [`scraper`] | Lower-level crawler actor implementation. |
//! | [`logging`] | Tracing and log subscriber initialization. |

/// Command-line and YAML configuration types.
pub mod cli;
/// Library error and result types.
pub mod error;
/// High-level crawler and file-scanner facades.
pub mod facade;
/// URL filtering primitives.
pub mod filter;
/// Secret extraction handlers.
pub mod handler;
/// Tracing/logging initialization helpers.
pub mod logging;
/// Human-readable and CSV output formatting.
pub mod output;
/// Per-domain crawler rate limiting.
pub mod rate_limiter;
/// Local file scanning engine.
pub mod scanner;
/// Lower-level crawler actor implementation.
pub mod scraper;
/// URL node and link extraction utilities.
pub mod urlparser;
