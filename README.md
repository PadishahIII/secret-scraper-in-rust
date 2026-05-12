# SecretScraper

SecretScraper is a Rust CLI and library for crawling web targets, discovering URLs and JavaScript links, and detecting secrets with regular-expression rules. It can also scan local files or directories recursively.

The package is prepared for use as both a CLI binary and a Rust library. Until a crates.io release is available, build, run, and depend on it from source.

**Library Doc**: <https://docs.rs/secret_scraper/latest/secret_scraper/>

## Features

- Crawl a single URL or newline-delimited URL seed file.
- Extract links from HTML (`a[href]`, `link[href]`, JavaScript scripts) and regex-based URL rules.
- Detect secrets with built-in and custom regex rules.
- Scan a single local file or a local directory tree.
- Allow-list and block-list domains with wildcard patterns.
- Configure headers, user agent, cookie, proxy, timeout, crawl depth, redirects, validation, and per-domain request limits.
- Write crawler results as CSV and local scan results as YAML.
- Use as a Rust library through `Config`, `CrawlerFacade`, `FileScannerFacade`, `ScanFacade`, and typed `SecretScraperError` results.



## Install

```bash
cargo install secret_scraper
```

This builds an optimized release binary and installs it as `secret_scraper` in your Cargo bin directory (typically `~/.cargo/bin`). Make sure that directory is on your `PATH`.

For development without installing, you can still use `cargo run --` in place of `secret_scraper` throughout the examples below.

## CLI Usage

```bash
secret_scraper --help
```

### Crawl One URL

```bash
secret_scraper --url https://example.com
```

### Crawl Multiple URLs

```bash
secret_scraper --url-file urls.txt
```

`urls.txt` is newline-delimited. Blank lines are ignored.

```text
https://example.com/
https://example.com/docs
https://example.org/
```

### Scan Local Files

```bash
secret_scraper --local ./samples
```

If `--local` points to a file, SecretScraper scans that file. If it points to a directory, files are scanned recursively.

### Write Output

Crawler output is CSV:

```bash
secret_scraper --url https://example.com --outfile crawl.csv
```

Local file scan output is YAML:

```bash
secret_scraper --local ./samples --outfile local-scan.yml
```

### Crawl Modes And Depth

```bash
secret_scraper --url https://example.com --mode normal
secret_scraper --url https://example.com --mode thorough
```

`normal` uses a crawl depth preset of `1`. `thorough` uses a crawl depth preset of `2`. If `--max-depth` is set, it overrides the mode preset.

```bash
secret_scraper --url https://example.com --mode thorough --max-depth 4
```

`--max-depth 0` fetches only the seed URL(s).

### Detail, Validation, Redirects, And Regex Output

Boolean CLI options are flags. Include the flag to enable the behavior; omit it to keep the default or YAML-configured value.

```bash
secret_scraper --url https://example.com --detail
secret_scraper --url https://example.com --validate
secret_scraper --url https://example.com --follow-redirect
secret_scraper --url https://example.com --hide-regex
```

`--validate` sends follow-up requests for discovered links to verify HTTP status. This can add requests even for links that are not crawled because of depth limits.

### Domain Filters

Allow-list and block-list filters accept comma-separated wildcard patterns:

```bash
secret_scraper --url https://example.com --allow-domains '*.example.com,example.org'
secret_scraper --url https://example.com --disallow-domains '*.gov,logout.example.com'
```

Filters apply to seed URLs and discovered URLs.

### Status Filters

Use `--status` with exact status codes and inclusive ranges:

```bash
secret_scraper --url https://example.com --status 200,300-399
```

### Headers, Cookie, User Agent, Proxy, And Rate Limits

```bash
secret_scraper \
  --url https://example.com \
  --ua "SecretScraper/0.1" \
  --cookie "session=abc" \
  --proxy "http://127.0.0.1:8080" \
  --max-concurrency-per-domain 10 \
  --min-request-interval 0.2
```

`--max-concurrency-per-domain` caps concurrent requests per domain. `--min-request-interval` is the minimum number of seconds between request starts for the same domain.

## CLI Options Summary

The authoritative list is `secret_scraper --help`. Current key options are:

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

The default config file path is `setting.yaml`. When that file does not exist, the binary writes a generated default configuration to that path and exits after printing a message.

### Using `setting.yaml`

Create a config file with the default path:

```bash
secret_scraper
```

Edit `setting.yaml`, then run the CLI again with the same default path:

```bash
secret_scraper
```

Use a different config file with `--config`:

```bash
secret_scraper --config showcase.yaml
```

CLI values override values loaded from YAML. For example, this uses all values from `showcase.yaml` except `url` and `outfile`:

```bash
secret_scraper --config showcase.yaml --url https://override.example --outfile override.csv
```

### Showcase Configuration

This example shows the expected shape of each configurable field with non-default demonstration values. Use it as a template, not as the generated default. The `urlFind`, `jsFind`, and `rules` entries shown here are custom additions; the generated `setting.yaml` already contains the built-in lists.

```yaml
debug: true
user_agent: "SecretScraper/0.1 (+https://example.local)"
cookie: "session=demo; theme=dark"
allow_domains:
  - "*.example.com"
  - "api.example.org"
disallow_domains:
  - "*.gov"
  - "logout.example.com"
url_file: "urls.txt"
config: "showcase.yaml"
timeout: 10.0
mode: thorough
max_page: 500
max_depth: 3
max_concurrent_per_domain: 10
min_request_interval: 0.5
outfile: "result.csv"
status_filter:
  - ["200", "200-400"]
proxy: "http://127.0.0.1:8080"
hide_regex: false
follow_redirects: true
dangerousPath:
  - logout
  - update
  - remove
  - insert
  - delete
url: "https://example.com"
detail: true
validate: true
local: null

headers:
  accept: "application/json,text/html,*/*"
  user-agent: "SecretScraper/0.1 (+https://example.local)"
  x-demo-header: "demo"

urlFind:
  - "https?://[A-Za-z0-9._~:/?#\\[\\]@!$&'()*+,;=%-]+"

jsFind:
  - "[\"']([^\"']+\\.js)[\"']"

rules:
  - name: Custom Secret
    regex: "SECRET_[A-Z0-9]+"
    loaded: true
    group: false
  - name: Disabled Rule
    regex: "IGNORE_ME"
    loaded: false
    group: false
```

For local scanning, replace the crawl target fields with `local`:

```yaml
url: null
url_file: null
local: "./samples"
outfile: "local-scan.yml"
```

### Default Configuration Values

The generated `setting.yaml` is the serialized form of `Config::default_with_rules()`. It includes all built-in URL, JavaScript, and secret rules.

The generated rule sections use two different shapes:

```yaml
urlFind:
  - "https?://..."
jsFind:
  - "[\"']([^\"']+\\.js)[\"']"
rules:
  - name: Custom Secret
    regex: "SECRET_[A-Z0-9]+"
    loaded: true
    group: false
headers:
  accept: "*/*"
  user-agent: "Mozilla/5.0 ..."
```

`urlFind` and `jsFind` are lists of regex strings that emit capture groups. `rules` is a list of named secret rules with `regex`, `loaded`, and `group` fields.

| Field | Default value | Meaning |
| --- | --- | --- |
| `debug` | `false` | Enable debug logging. |
| `user_agent` | `null` | Optional user-agent override. When set, it is inserted into request headers. |
| `cookie` | `null` | Optional cookie header value. |
| `allow_domains` | `null` | Optional allow-list of wildcard domain patterns. |
| `disallow_domains` | `null` | Optional block-list of wildcard domain patterns. |
| `url_file` | `null` | Optional newline-delimited seed URL file. |
| `config` | `setting.yaml` | Config file path used by the CLI. |
| `timeout` | `30.0` | Request timeout in seconds. |
| `mode` | `normal` | Crawl mode preset. `normal` uses depth 1; `thorough` uses depth 2. |
| `max_page` | `1000` | Maximum number of pages to crawl. |
| `max_depth` | `null` | Optional explicit crawl depth override. `0` means seed URLs only. |
| `max_concurrent_per_domain` | `50` | Maximum concurrent requests per domain. |
| `min_request_interval` | `0.2` | Minimum seconds between requests to the same domain. |
| `outfile` | `null` | Optional output path. Crawl output is CSV; local scan output is YAML. |
| `status_filter` | `null` | Optional response status display filter. |
| `proxy` | `null` | Optional proxy URL such as `http://127.0.0.1:8080` or `socks5://127.0.0.1:7890`. |
| `hide_regex` | `false` | Hide regex/secret output in human-readable output. |
| `follow_redirects` | `false` | Follow HTTP redirects while crawling. |
| `dangerousPath` | `null` | Optional path fragments to avoid requesting. |
| `url` | `null` | Optional single crawl seed URL. |
| `detail` | `false` | Show detailed crawl output. |
| `validate` | `false` | Validate discovered link status after crawling. |
| `local` | `null` | Optional local file or directory to scan recursively. |
| `urlFind` | five built-in regex strings | Regex rules used to discover URLs from text. |
| `jsFind` | three built-in regex strings | Regex rules used to discover JavaScript URLs from text. |
| `rules` | ten built-in named secret rules | Regex rules used to detect secrets. |
| `headers` | `accept: "*/*"` and `user-agent: "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/80.0.3987.87 Safari/537.36 SE 2.X MetaSr 1.0"` | Default HTTP headers sent by crawler requests. |

### Field Notes

- At least one of `url`, `url_file`, or `local` must be set before scanning.
- `follow_redirects` is the YAML field name; the CLI flag is `--follow-redirect`.
- `headers` values are merged onto the default header map. Reusing `accept` or `user-agent` overrides the default values.
- `urlFind` and `jsFind` entries are plain regex strings. They do not use `name` or `loaded`, and they emit capture groups by default.
- `rules` entries use `name`, `regex`, `loaded`, and optional `group`. `group: true` emits capture groups instead of the full match; omitted `group` defaults to `false`.
- `urlFind`, `jsFind`, and loaded `rules` entries are appended to any existing rules when you apply a YAML layer to `Config::default_with_rules()`.


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

SecretScraper can be used directly from Rust code. Before the first crates.io release, depend on it by local path while developing.

```toml
[dependencies]
secret_scraper = { path = "../secret-scraper-in-rust" }
```

After publication, use the crates.io dependency form:

```toml
[dependencies]
secret_scraper = "0.1"
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
