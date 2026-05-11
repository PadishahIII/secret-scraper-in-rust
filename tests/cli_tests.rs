use clap::{CommandFactory, Parser, error::ErrorKind};
use secret_scraper::cli::{CliConfigLayer, Config, Mode};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn parse(args: &[&str]) -> CliConfigLayer {
    let mut argv = vec!["secret-scraper"];
    argv.extend_from_slice(args);
    CliConfigLayer::try_parse_from(argv).expect("valid CLI args")
}

fn parse_error(args: &[&str]) -> ErrorKind {
    let mut argv = vec!["secret-scraper"];
    argv.extend_from_slice(args);
    CliConfigLayer::try_parse_from(argv)
        .expect_err("invalid CLI args")
        .kind()
}

fn unique_temp_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("secret_scraper_cli_tests_{name}_{nanos}"))
}

fn temp_file(name: &str, content: &str) -> PathBuf {
    let path = unique_temp_path(name);
    fs::write(&path, content).expect("write temp file");
    path
}

fn remove_file_if_exists(path: &Path) {
    let _ = fs::remove_file(path);
}

#[test]
fn clap_definition_is_internally_consistent() {
    CliConfigLayer::command().debug_assert();
}

#[test]
fn help_and_version_are_handled_by_clap() {
    assert_eq!(parse_error(&["--help"]), ErrorKind::DisplayHelp);
    assert_eq!(parse_error(&["--version"]), ErrorKind::DisplayVersion);
}

#[test]
fn no_cli_options_uses_default_config_path_and_leaves_optional_fields_empty() {
    let cli = parse(&[]);

    assert_eq!(cli.config, PathBuf::from("setting.yaml"));
    assert_eq!(cli.debug, Some(false));
    assert_eq!(cli.verbose, Some(false));
    assert_eq!(cli.user_agent, None);
    assert_eq!(cli.cookie, None);
    assert_eq!(cli.allow_domains, None);
    assert_eq!(cli.disallow_domains, None);
    assert_eq!(cli.url_file, None);
    assert!(cli.mode.is_none());
    assert_eq!(cli.max_page, None);
    assert_eq!(cli.max_depth, None);
    assert_eq!(cli.max_concurrency_per_domain, None);
    assert_eq!(cli.min_request_interval, None);
    assert_eq!(cli.outfile, None);
    assert!(cli.status_filter.is_none());
    assert_eq!(cli.proxy, None);
    assert_eq!(cli.hide_regex, Some(false));
    assert_eq!(cli.follow_redirect, Some(false));
    assert_eq!(cli.url, None);
    assert_eq!(cli.detail, Some(false));
    assert_eq!(cli.validate, Some(false));
    assert_eq!(cli.local, None);
}

#[test]
fn parses_debug_flag() {
    assert_eq!(parse(&["--debug"]).debug, Some(true));
}

#[test]
fn parses_verbose_flag() {
    assert_eq!(parse(&["--verbose"]).verbose, Some(true));
}

#[test]
fn parses_user_agent_long_and_short_options() {
    assert_eq!(
        parse(&["--ua", "cli-agent"]).user_agent.as_deref(),
        Some("cli-agent")
    );
    assert_eq!(
        parse(&["-a", "short-agent"]).user_agent.as_deref(),
        Some("short-agent")
    );
}

#[test]
fn parses_cookie_long_and_short_options() {
    assert_eq!(
        parse(&["--cookie", "session=abc"]).cookie.as_deref(),
        Some("session=abc")
    );
    assert_eq!(
        parse(&["-c", "token=xyz"]).cookie.as_deref(),
        Some("token=xyz")
    );
}

#[test]
fn parses_allow_domains_long_and_short_options() {
    assert_eq!(
        parse(&["--allow-domains", "*.example.com,api.example.org"])
            .allow_domains
            .as_deref(),
        Some(&["*.example.com".to_string(), "api.example.org".to_string()][..])
    );
    assert_eq!(
        parse(&["-d", "example.com,example.net"])
            .allow_domains
            .as_deref(),
        Some(&["example.com".to_string(), "example.net".to_string()][..])
    );
}

#[test]
fn parses_disallow_domains_long_and_short_options() {
    assert_eq!(
        parse(&["--disallow-domains", "*.gov,logout.example.com"])
            .disallow_domains
            .as_deref(),
        Some(&["*.gov".to_string(), "logout.example.com".to_string()][..])
    );
    assert_eq!(
        parse(&["-D", "bad.example,*.invalid"])
            .disallow_domains
            .as_deref(),
        Some(&["bad.example".to_string(), "*.invalid".to_string()][..])
    );
}

#[test]
fn parses_existing_url_file_long_and_short_options() {
    let path = temp_file("urls", "https://example.com\n");

    assert_eq!(
        parse(&["--url-file", path.to_str().unwrap()])
            .url_file
            .as_deref(),
        Some(path.as_path())
    );
    assert_eq!(
        parse(&["-f", path.to_str().unwrap()]).url_file.as_deref(),
        Some(path.as_path())
    );

    remove_file_if_exists(&path);
}

#[test]
fn rejects_missing_url_file() {
    let missing = unique_temp_path("missing_urls");

    assert_eq!(
        parse_error(&["--url-file", missing.to_str().unwrap()]),
        ErrorKind::ValueValidation
    );
}

#[test]
fn parses_config_long_and_short_options() {
    assert_eq!(
        parse(&["--config", "custom.yaml"]).config,
        PathBuf::from("custom.yaml")
    );
    assert_eq!(
        parse(&["-i", "short.yaml"]).config,
        PathBuf::from("short.yaml")
    );
}

#[test]
fn parses_mode_long_and_short_options() {
    assert!(matches!(
        parse(&["--mode", "normal"]).mode,
        Some(Mode::Normal)
    ));
    assert!(matches!(
        parse(&["-m", "thorough"]).mode,
        Some(Mode::Thorough)
    ));
}

#[test]
fn rejects_invalid_mode() {
    assert_eq!(parse_error(&["--mode", "fast"]), ErrorKind::InvalidValue);
}

#[test]
fn parses_max_page_option() {
    assert_eq!(parse(&["--max-page", "42"]).max_page, Some(42));
}

#[test]
fn parses_max_depth_option_including_zero() {
    assert_eq!(parse(&["--max-depth", "0"]).max_depth, Some(0));
    assert_eq!(parse(&["--max-depth", "4"]).max_depth, Some(4));
}

#[test]
fn parses_max_concurrency_per_domain_option() {
    assert_eq!(
        parse(&["--max-concurrency-per-domain", "12"]).max_concurrency_per_domain,
        Some(12)
    );
}

#[test]
fn parses_min_request_interval_option() {
    assert_eq!(
        parse(&["--min-request-interval", "0.75"]).min_request_interval,
        Some(0.75)
    );
}

#[test]
fn parses_outfile_long_and_short_options() {
    assert_eq!(
        parse(&["--outfile", "crawl.csv"]).outfile.as_deref(),
        Some(Path::new("crawl.csv"))
    );
    assert_eq!(
        parse(&["-o", "local.yml"]).outfile.as_deref(),
        Some(Path::new("local.yml"))
    );
}

#[test]
fn parses_status_filter_long_and_short_options() {
    let long = parse(&["--status", "200,300-399"]);
    let long_filter = long.status_filter.as_ref().expect("status filter");
    assert!(long_filter.is_allowed(200));
    assert!(long_filter.is_allowed(302));
    assert!(!long_filter.is_allowed(404));

    let short = parse(&["-s", "201"]);
    let short_filter = short.status_filter.as_ref().expect("status filter");
    assert!(short_filter.is_allowed(201));
    assert!(!short_filter.is_allowed(200));
}

#[test]
fn rejects_invalid_status_filter() {
    assert_eq!(
        parse_error(&["--status", "abc"]),
        ErrorKind::ValueValidation
    );
    assert_eq!(
        parse_error(&["--status", "200-abc"]),
        ErrorKind::ValueValidation
    );
}

#[test]
fn parses_proxy_long_and_short_options() {
    assert_eq!(
        parse(&["--proxy", "http://127.0.0.1:8080"])
            .proxy
            .as_deref(),
        Some("http://127.0.0.1:8080")
    );
    assert_eq!(
        parse(&["-x", "socks5://127.0.0.1:7890"]).proxy.as_deref(),
        Some("socks5://127.0.0.1:7890")
    );
}

#[test]
fn parses_boolean_runtime_flags() {
    let cli = parse(&[
        "--hide-regex",
        "--follow-redirect",
        "--detail",
        "--validate",
    ]);

    assert_eq!(cli.hide_regex, Some(true));
    assert_eq!(cli.follow_redirect, Some(true));
    assert_eq!(cli.detail, Some(true));
    assert_eq!(cli.validate, Some(true));
}

#[test]
fn parses_boolean_short_flags() {
    let cli = parse(&["-H", "-F"]);

    assert_eq!(cli.hide_regex, Some(true));
    assert_eq!(cli.follow_redirect, Some(true));
}

#[test]
fn absent_boolean_flags_do_not_override_yaml_layer_values() {
    let yaml = r#"
urlFind: []
jsFind: []
url: "https://yaml.example"
debug: true
hide_regex: true
follow_redirect: true
detail: true
validate: true
"#;
    let layer = serde_yaml::from_str(yaml).expect("valid yaml");
    let mut config = Config::default();
    config.apply_file_layer(layer).expect("valid yaml layer");
    config.apply_cli_layer(parse(&["--url", "https://cli.example"]));

    assert_eq!(config.url.as_deref(), Some("https://cli.example"));
    assert!(config.debug);
    assert!(config.hide_regex);
    assert!(config.follow_redirect);
    assert!(config.detail);
    assert!(config.validate);
}

#[test]
fn boolean_options_reject_explicit_values() {
    assert_eq!(
        parse_error(&["--debug", "true"]),
        ErrorKind::UnknownArgument
    );
    assert_eq!(
        parse_error(&["--verbose", "true"]),
        ErrorKind::UnknownArgument
    );
    assert_eq!(
        parse_error(&["--hide-regex", "true"]),
        ErrorKind::UnknownArgument
    );
    assert_eq!(
        parse_error(&["--follow-redirect", "true"]),
        ErrorKind::UnknownArgument
    );
    assert_eq!(
        parse_error(&["--detail", "true"]),
        ErrorKind::UnknownArgument
    );
    assert_eq!(
        parse_error(&["--validate", "true"]),
        ErrorKind::UnknownArgument
    );
}

#[test]
fn parses_url_long_and_short_options() {
    assert_eq!(
        parse(&["--url", "https://example.com"]).url.as_deref(),
        Some("https://example.com")
    );
    assert_eq!(
        parse(&["-u", "https://short.example"]).url.as_deref(),
        Some("https://short.example")
    );
}

#[test]
fn parses_existing_local_path_long_and_short_options() {
    let path = temp_file("local", "AKIA1234567890ABCDEF\n");

    assert_eq!(
        parse(&["--local", path.to_str().unwrap()]).local.as_deref(),
        Some(path.as_path())
    );
    assert_eq!(
        parse(&["-l", path.to_str().unwrap()]).local.as_deref(),
        Some(path.as_path())
    );

    remove_file_if_exists(&path);
}

#[test]
fn rejects_missing_local_path() {
    let missing = unique_temp_path("missing_local");

    assert_eq!(
        parse_error(&["--local", missing.to_str().unwrap()]),
        ErrorKind::ValueValidation
    );
}

#[test]
fn common_url_crawl_options_merge_into_runtime_config() {
    let cli = parse(&[
        "--url",
        "https://example.com",
        "--mode",
        "thorough",
        "--max-depth",
        "3",
        "--max-page",
        "50",
        "--detail",
        "--validate",
        "--follow-redirect",
        "--hide-regex",
        "--allow-domains",
        "*.example.com,api.example.org",
        "--disallow-domains",
        "logout.example.com",
        "--status",
        "200,300-399",
        "--ua",
        "SecretScraper/0.1",
        "--cookie",
        "session=abc",
        "--proxy",
        "http://127.0.0.1:8080",
        "--max-concurrency-per-domain",
        "10",
        "--min-request-interval",
        "0.5",
        "--outfile",
        "crawl.csv",
    ]);
    let mut config = Config::default();
    config.apply_cli_layer(cli);

    assert_eq!(config.url.as_deref(), Some("https://example.com"));
    assert!(matches!(config.mode, Mode::Thorough));
    assert_eq!(config.max_depth, Some(3));
    assert_eq!(config.max_page, Some(50));
    assert!(config.detail);
    assert!(config.validate);
    assert!(config.follow_redirect);
    assert!(config.hide_regex);
    assert_eq!(
        config.allow_domains.as_deref(),
        Some(&["*.example.com".to_string(), "api.example.org".to_string()][..])
    );
    assert_eq!(
        config.disallow_domains.as_deref(),
        Some(&["logout.example.com".to_string()][..])
    );
    let filter = config.status_filter.as_ref().expect("status filter");
    assert!(filter.is_allowed(200));
    assert!(filter.is_allowed(399));
    assert!(!filter.is_allowed(404));
    assert_eq!(config.user_agent.as_deref(), Some("SecretScraper/0.1"));
    assert_eq!(config.cookie.as_deref(), Some("session=abc"));
    assert_eq!(config.proxy.as_deref(), Some("http://127.0.0.1:8080"));
    assert_eq!(config.max_concurrency_per_domain, 10);
    assert_eq!(config.min_request_interval.as_millis(), 500);
    assert_eq!(config.outfile.as_deref(), Some(Path::new("crawl.csv")));
    config.validate().expect("url target is valid");
}

#[test]
fn common_url_file_use_case_validates_after_cli_merge() {
    let path = temp_file(
        "url_file_use_case",
        "https://example.com\nhttps://example.org\n",
    );
    let cli = parse(&[
        "--url-file",
        path.to_str().unwrap(),
        "--outfile",
        "crawl.csv",
    ]);
    let mut config = Config::default();
    config.apply_cli_layer(cli);

    assert_eq!(config.url_file.as_deref(), Some(path.as_path()));
    assert_eq!(config.outfile.as_deref(), Some(Path::new("crawl.csv")));
    config.validate().expect("url file target is valid");

    remove_file_if_exists(&path);
}

#[test]
fn common_local_scan_use_case_validates_after_cli_merge() {
    let path = temp_file("local_scan_use_case", "SECRET_TOKEN_1234567890\n");
    let cli = parse(&[
        "--local",
        path.to_str().unwrap(),
        "--outfile",
        "local-scan.yml",
    ]);
    let mut config = Config::default();
    config.apply_cli_layer(cli);

    assert_eq!(config.local.as_deref(), Some(path.as_path()));
    assert_eq!(config.outfile.as_deref(), Some(Path::new("local-scan.yml")));
    config.validate().expect("local scan target is valid");

    remove_file_if_exists(&path);
}

#[test]
fn common_config_override_use_case_keeps_cli_values_authoritative() {
    let cli = parse(&[
        "--config",
        "showcase.yaml",
        "--url",
        "https://override.example",
        "--outfile",
        "override.csv",
    ]);
    let mut config = Config::default();
    config.apply_cli_layer(cli);

    assert_eq!(config.config, PathBuf::from("showcase.yaml"));
    assert_eq!(config.url.as_deref(), Some("https://override.example"));
    assert_eq!(config.outfile.as_deref(), Some(Path::new("override.csv")));
    config.validate().expect("override url target is valid");
}

#[test]
fn validation_rejects_missing_target_after_successful_cli_parse() {
    let cli = parse(&["--debug"]);
    let mut config = Config::default();
    config.apply_cli_layer(cli);

    let error = config.validate().expect_err("target is required");
    assert!(
        error
            .to_string()
            .contains("At least one of --url, --url-file, or --local must be specified")
    );
}
