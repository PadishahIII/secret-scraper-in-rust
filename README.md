# SecretScraper

SecretScraper is a Rust CLI and library for crawling web targets, discovering URLs and JavaScript links, and detecting secrets with regular-expression rules. It can also scan local files or directories recursively.

The project is not published as a public package yet. Build, run, and use it from source.

## Features

- Crawl a single URL or newline-delimited URL seed file.
- Extract links from HTML (`a[href]`, `link[href]`, JavaScript scripts) and regex-based URL rules.
- Detect secrets with built-in and custom regex rules.
- Scan a single local file or a local directory tree.
- Allow-list and block-list domains with wildcard patterns.
- Configure headers, user agent, cookie, proxy, timeout, crawl depth, redirects, validation, and per-domain request limits.
- Write crawler results as CSV and local scan results as YAML.
- Use as a Rust library through `Config`, `CrawlerFacade`, `FileScannerFacade`, `ScanFacade`, and typed `SecretScraperError` results.

## Build From Source

```bash
cargo build
```

For an optimized binary:

```bash
cargo build --release
```

To install the current checkout locally:

```bash
cargo install --path .
```

## CLI Usage

Run the current help text:

```bash
cargo run -- --help
```

After `cargo install --path .`, replace `cargo run --` with `secret_scraper`.

### Crawl One URL

```bash
cargo run -- --url https://example.com
```

### Crawl Multiple URLs

```bash
cargo run -- --url-file urls.txt
```

`urls.txt` is newline-delimited. Blank lines are ignored.

```text
https://example.com/
https://example.com/docs
https://example.org/
```

### Scan Local Files

```bash
cargo run -- --local ./samples
```

If `--local` points to a file, SecretScraper scans that file. If it points to a directory, files are scanned recursively.

### Write Output

Crawler output is CSV:

```bash
cargo run -- --url https://example.com --outfile crawl.csv
```

Local file scan output is YAML:

```bash
cargo run -- --local ./samples --outfile local-scan.yml
```

### Crawl Modes And Depth

```bash
cargo run -- --url https://example.com --mode normal
cargo run -- --url https://example.com --mode thorough
```

`normal` uses a crawl depth preset of `1`. `thorough` uses a crawl depth preset of `2`. If `--max-depth` is set, it overrides the mode preset.

```bash
cargo run -- --url https://example.com --mode thorough --max-depth 4
```

`--max-depth 0` fetches only the seed URL(s).

### Detail, Validation, Redirects, And Regex Output

Boolean CLI options currently take explicit `true` or `false` values:

```bash
cargo run -- --url https://example.com --detail true
cargo run -- --url https://example.com --validate true
cargo run -- --url https://example.com --follow-redirect true
cargo run -- --url https://example.com --hide-regex true
```

`--validate true` sends follow-up requests for discovered links to verify HTTP status. This can add requests even for links that are not crawled because of depth limits.

### Domain Filters

Allow-list and block-list filters accept comma-separated wildcard patterns:

```bash
cargo run -- --url https://example.com --allow-domains '*.example.com,example.org'
cargo run -- --url https://example.com --disallow-domains '*.gov,logout.example.com'
```

Filters apply to seed URLs and discovered URLs.

### Status Filters

Use `--status` with exact status codes and inclusive ranges:

```bash
cargo run -- --url https://example.com --status 200,300-399
```

### Headers, Cookie, User Agent, Proxy, And Rate Limits

```bash
cargo run -- \
  --url https://example.com \
  --ua "SecretScraper/0.1" \
  --cookie "session=abc" \
  --proxy "http://127.0.0.1:8080" \
  --max-concurrency-per-domain 10 \
  --min-request-interval 0.2
```

`--max-concurrency-per-domain` caps concurrent requests per domain. `--min-request-interval` is the minimum number of seconds between request starts for the same domain.

## CLI Options Summary

The authoritative list is `cargo run -- --help`. Current key options are:

- `--url`, `-u`: crawl one seed URL.
- `--url-file`, `-f`: crawl seed URLs from a newline-delimited file.
- `--local`, `-l`: scan a local file or directory recursively.
- `--config`, `-i`: load a YAML config file.
- `--mode`, `-m`: `normal` or `thorough`.
- `--max-page`: maximum number of pages to crawl.
- `--max-depth`: explicit crawl depth override.
- `--max-concurrency-per-domain`: max concurrent requests per domain.
- `--min-request-interval`: seconds between requests to the same domain.
- `--outfile`, `-o`: write crawler CSV or local scan YAML output.
- `--status`, `-s`: filter displayed response statuses.
- `--allow-domains`, `-d`: allow-list domains.
- `--disallow-domains`, `-D`: block-list domains.
- `--ua`, `-a`: set `User-Agent`.
- `--cookie`, `-c`: set `Cookie`.
- `--proxy`, `-x`: set HTTP/SOCKS proxy.
- `--debug`: enable debug logging.
- `--detail`: print detailed crawl output.
- `--validate`: validate discovered link statuses.
- `--follow-redirect`, `-F`: follow redirects.
- `--hide-regex`, `-H`: hide regex/secret output.

At least one of `--url`, `--url-file`, or `--local` is required.

## Configuration

The runtime configuration is built in this order:

1. Start from `Config::default()` or `Config::default_with_rules()`.
2. Apply YAML with `Config::apply_file_layer(...)`.
3. Apply CLI options with `Config::apply_cli_layer(...)`.
4. Validate with `Config::validate()`.

CLI values override YAML values. Missing CLI/YAML fields do not clear existing values.

When the binary starts without a config file, it writes a default `settings.yaml` in the current directory and exits after printing a message.

### YAML Example

```yaml
debug: false
url: "https://example.com"
mode: normal
max_page: 100
max_depth: 2
detail: true
validate: false
outfile: "result.csv"

timeout: 30
max_page_num: 100000
max_concurrent_per_domain: 50
min_request_interval: 0.2
follow_redirects: false

allow_domains:
  - "*.example.com"
disallow_domains:
  - "*.gov"
dangerousPath:
  - logout
  - update
  - remove
  - insert
  - delete

headers:
  Accept: "*/*"
  Cookie: ""
  User-Agent: "Mozilla/5.0"

urlFind:
  - "[\"'‘“`]\\s{0,6}(https{0,1}:[-a-zA-Z0-9()@:%_\\+.~#?&//={}]{2,100}?)\\s{0,6}[\"''‘“`]'"

jsFind:
  - "(https{0,1}:[-a-zA-Z0-9（）@:%_\\+.~#?&//=]{2,100}?[-a-zA-Z0-9（）@:%_\\+.~#?&//=]{3}[.]js)"

rules:
  - name: Custom Secret
    regex: "SECRET_[A-Z0-9]+"
    loaded: true
  - name: Disabled Rule
    regex: "IGNORE_ME"
    loaded: false
```

YAML-only keys include `timeout`, `max_concurrent_per_domain`, `follow_redirects`, `max_page_num`, `dangerousPath`, `headers`, `urlFind`, `jsFind`, and `rules`.

`rules` entries with `loaded: false` are skipped. Invalid regex patterns cause configuration loading to fail.

### Built-In Rules

Use `Config::default_with_rules()` to populate built-in URL, JavaScript, and secret-detection rules. The built-in custom secret rules currently include:

- Swagger
- ID Card
- Phone
- JS Map
- URL as a value
- Email
- Internal IP
- Cloud Key
- Shiro
- Suspicious API Key

YAML `urlFind`, `jsFind`, and loaded `rules` entries are appended to the existing rule lists.

## Library Usage

SecretScraper can be used directly from Rust code. The crate is not published yet, so depend on it by local path while developing.

```toml
[dependencies]
secret_scraper = { path = "../secret-scraper-in-rust" }
```

### Crawl From Library Code

```rust
use secret_scraper::{
    cli::{Config, Mode},
    error::Result as SecretScraperResult,
    facade::{CrawlerFacade, ScanFacade, ScanResult},
};

fn crawl() -> SecretScraperResult<()> {
    let mut config = Config::default_with_rules();
    config.url = Some("https://example.com".to_string());
    config.mode = Mode::Thorough;
    config.detail = true;
    config.outfile = Some("crawl.csv".into());

    match Box::new(CrawlerFacade::new(config)?).scan()? {
        ScanResult::CrawlResult(result) => {
            println!(
                "{} domains, {} URL groups, {} secret-bearing URLs",
                result.hosts.len(),
                result.urls.len(),
                result.secrets.len()
            );
        }
        ScanResult::LocalScanResult(_) => unreachable!("CrawlerFacade returns crawl results"),
    }

    Ok(())
}
```

### Scan Local Files From Library Code

```rust
use secret_scraper::{
    cli::Config,
    error::Result as SecretScraperResult,
    facade::{FileScannerFacade, ScanFacade, ScanResult},
};

fn scan_local() -> SecretScraperResult<()> {
    let mut config = Config::default_with_rules();
    config.local = Some("./samples".into());
    config.outfile = Some("local-scan.yml".into());

    match Box::new(FileScannerFacade::new(config)?).scan()? {
        ScanResult::LocalScanResult(result) => {
            println!("{} files scanned", result.len());
        }
        ScanResult::CrawlResult(_) => unreachable!("FileScannerFacade returns local scan results"),
    }

    Ok(())
}
```

The public facade result type is `ScanStdResult`, an alias for `secret_scraper::error::Result<ScanResult>`. Errors are represented by `SecretScraperError`.

## Examples

Run the local crawler example:

```bash
cargo run --example crawler
```

Run the local file-scanner facade example:

```bash
cargo run --example scan_facade
```

The crawler example starts a local HTTP server and runs both normal and thorough crawls. The scan facade example creates temporary files, scans them, writes YAML, and handles `SecretScraperResult` explicitly.

## Development

```bash
cargo fmt --check
cargo check
cargo check --examples
cargo test
```

Generate docs locally:

```bash
cargo doc --no-deps --open
```

## Notes

- Crawler output files are CSV.
- Local scan output files are YAML.
- The project currently uses Rust `regex` for rule matching.
- Public package links are intentionally omitted until the crate is published.
