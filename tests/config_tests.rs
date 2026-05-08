use secret_scraper::cli::*;
use std::path::PathBuf;

fn fully_specified_cli_layer() -> CliConfigLayer {
    CliConfigLayer {
        debug: Some(true),
        user_agent: Some("cli-ua".into()),
        cookie: Some("cli-cookie".into()),
        allow_domains: Some(vec!["example.com".into()]),
        disallow_domains: Some(vec!["bad.com".into()]),
        url_file: Some(PathBuf::from("urls.txt")),
        config: Some(PathBuf::from("custom.yaml")),
        mode: Some(Mode::Thorough),
        max_page: Some(42),
        max_connections: Some(8),
        max_keepalive_connections: Some(4),
        max_concurrency_per_domain: Some(16),
        min_request_interval: Some(0.5),
        outfile: Some(PathBuf::from("out.csv")),
        status_filter: Some(vec![StatusRange::Exact(200), StatusRange::Range(300, 399)].into()),
        proxy: Some("http://proxy:8080".into()),
        hide_regex: Some(true),
        follow_redirect: Some(true),
        url: Some("https://cli-target.example".into()),
        detail: Some(true),
        validate: Some(true),
        local: Some(PathBuf::from("/tmp/scan")),
    }
}

fn assert_config_matches_fully_specified_cli(config: &Config) {
    assert!(config.debug);
    assert_eq!(config.user_agent.as_deref(), Some("cli-ua"));
    assert_eq!(config.cookie.as_deref(), Some("cli-cookie"));
    assert_eq!(
        config.allow_domains.as_deref(),
        Some(&["example.com".to_string()][..])
    );
    assert_eq!(
        config.disallow_domains.as_deref(),
        Some(&["bad.com".to_string()][..])
    );
    assert_eq!(
        config.url_file.as_deref(),
        Some(PathBuf::from("urls.txt").as_path())
    );
    assert_eq!(
        config.config.as_deref(),
        Some(PathBuf::from("custom.yaml").as_path())
    );
    assert!(matches!(config.mode, Mode::Thorough));
    assert_eq!(config.max_page, 42);
    assert_eq!(config.max_connections, 8);
    assert_eq!(config.max_keepalive_connections, 4);
    assert_eq!(config.max_concurrency_per_domain, Some(16));
    assert_eq!(config.min_request_interval, Some(0.5));
    assert_eq!(
        config.outfile.as_deref(),
        Some(PathBuf::from("out.csv").as_path())
    );
    let _status_filter = config.status_filter.as_ref().expect("status filter");
    assert_eq!(config.proxy.as_deref(), Some("http://proxy:8080"));
    assert!(config.hide_regex);
    assert!(config.follow_redirect);
    assert_eq!(config.url, "https://cli-target.example");
    assert!(config.detail);
    assert!(config.validate);
    assert_eq!(
        config.local.as_deref(),
        Some(PathBuf::from("/tmp/scan").as_path())
    );
}

fn yaml_layer_from_str(yaml: &str) -> FileConfigLayer {
    let mut full_yaml = String::new();
    if !yaml.contains("urlFind:") {
        full_yaml.push_str("urlFind: []\n");
    }
    if !yaml.contains("jsFind:") {
        full_yaml.push_str("jsFind: []\n");
    }
    full_yaml.push_str(yaml);
    serde_yaml::from_str(&full_yaml).expect("valid yaml")
}

#[test]
fn default_config_values() {
    let cfg = Config::default();
    assert!(!cfg.debug);
    assert!(!cfg.hide_regex);
    assert!(!cfg.follow_redirect);
    assert!(!cfg.detail);
    assert!(!cfg.validate);
    assert_eq!(cfg.user_agent, None);
    assert_eq!(cfg.cookie, None);
    assert_eq!(cfg.proxy, None);
    assert_eq!(cfg.allow_domains, None);
    assert_eq!(cfg.disallow_domains, None);
    assert_eq!(cfg.url_file, None);
    assert_eq!(cfg.config, None);
    assert_eq!(cfg.outfile, None);
    assert_eq!(cfg.local, None);
    assert!(matches!(cfg.mode, Mode::Normal));
    assert_eq!(cfg.max_page, 100_000);
    assert_eq!(cfg.max_connections, 100);
    assert_eq!(cfg.max_keepalive_connections, 50);
    assert_eq!(cfg.max_concurrency_per_domain, None);
    assert_eq!(cfg.min_request_interval, None);
    assert!(cfg.status_filter.is_none());
    assert_eq!(cfg.url, "");
    assert!(cfg.url_find_rules.is_empty());
    assert!(cfg.js_find_rules.is_empty());
    assert!(cfg.custom_rules.is_empty());
}

#[test]
fn default_with_rules_populated() {
    let cfg = Config::default_with_rules();
    assert!(!cfg.url_find_rules.is_empty());
    assert!(!cfg.js_find_rules.is_empty());
    assert!(!cfg.custom_rules.is_empty());
    for rule in &cfg.url_find_rules {
        assert!(!rule.name.is_empty());
    }
    for rule in &cfg.js_find_rules {
        assert!(!rule.name.is_empty());
    }
    for rule in &cfg.custom_rules {
        assert!(!rule.name.is_empty());
    }
    assert_eq!(cfg.max_page, 100_000);
    assert_eq!(cfg.max_connections, 100);
}

#[test]
fn cli_layer_overrides_all_default_fields() {
    let mut cfg = Config::default();
    let cli = fully_specified_cli_layer();
    cfg.apply_cli_layer(cli);
    assert_config_matches_fully_specified_cli(&cfg);
}

#[test]
fn cli_layer_partial_only_overrides_specified_fields() {
    let mut cfg = Config::default_with_rules();
    let cli = CliConfigLayer {
        debug: Some(true),
        max_page: Some(500),
        url: Some("https://partial.example".into()),
        ..Default::default()
    };
    cfg.apply_cli_layer(cli);
    assert!(cfg.debug);
    assert_eq!(cfg.max_page, 500);
    assert_eq!(cfg.url, "https://partial.example");
    assert!(!cfg.follow_redirect);
    assert_eq!(cfg.max_connections, 100);
    assert_eq!(cfg.max_keepalive_connections, 50);
    assert_eq!(cfg.user_agent, None);
    assert_eq!(cfg.proxy, None);
    assert!(!cfg.url_find_rules.is_empty());
    assert!(!cfg.custom_rules.is_empty());
}

#[test]
fn empty_cli_layer_changes_nothing() {
    let mut cfg = Config::default_with_rules();
    let orig_page = cfg.max_page;
    let orig_rules = cfg.custom_rules.len();
    cfg.apply_cli_layer(CliConfigLayer::default());
    assert_eq!(cfg.max_page, orig_page);
    assert_eq!(cfg.custom_rules.len(), orig_rules);
    assert!(!cfg.debug);
    assert_eq!(cfg.url, "");
}

#[test]
fn cli_option_field_none_does_not_clear_existing_value() {
    let mut cfg = Config::default();
    cfg.apply_cli_layer(CliConfigLayer {
        user_agent: Some("first-ua".into()),
        proxy: Some("first-proxy".into()),
        ..Default::default()
    });
    assert_eq!(cfg.user_agent.as_deref(), Some("first-ua"));
    assert_eq!(cfg.proxy.as_deref(), Some("first-proxy"));
    cfg.apply_cli_layer(CliConfigLayer {
        max_page: Some(999),
        ..Default::default()
    });
    assert_eq!(cfg.user_agent.as_deref(), Some("first-ua"));
    assert_eq!(cfg.proxy.as_deref(), Some("first-proxy"));
    assert_eq!(cfg.max_page, 999);
}

#[test]
fn cli_bool_fields_toggle_correctly() {
    let mut cfg = Config::default();
    cfg.apply_cli_layer(CliConfigLayer {
        debug: Some(true),
        ..Default::default()
    });
    assert!(cfg.debug);
    cfg.apply_cli_layer(CliConfigLayer {
        hide_regex: Some(true),
        ..Default::default()
    });
    assert!(cfg.hide_regex);
    cfg.apply_cli_layer(CliConfigLayer {
        follow_redirect: Some(true),
        ..Default::default()
    });
    assert!(cfg.follow_redirect);
    assert!(cfg.debug);
}

#[test]
fn yaml_deserialization_empty() {
    let layer: FileConfigLayer =
        serde_yaml::from_str("urlFind: []\njsFind: []").expect("empty yaml");
    assert!(layer.url_find_rules.is_empty());
    assert!(layer.js_find_rules.is_empty());
    assert!(layer.rules.is_none());
    assert_eq!(layer.cli_options.debug, None);
    assert_eq!(layer.cli_options.url, None);
}

#[test]
fn yaml_deserialization_full() {
    let yaml = r#"
debug: true
url: "https://yaml-target.example"
max_page: 999
urlFind:
  - "url_pattern_1"
  - "url_pattern_2"
jsFind:
  - "js_pattern_1"
rules:
  - name: "CustomRule"
    regex: "secret_\\d+"
    loaded: true
  - name: "DisabledRule"
    regex: "ignore_me"
    loaded: false
"#;
    let layer: FileConfigLayer = serde_yaml::from_str(yaml).expect("full yaml");
    assert_eq!(layer.cli_options.debug, Some(true));
    assert_eq!(
        layer.cli_options.url.as_deref(),
        Some("https://yaml-target.example")
    );
    assert_eq!(layer.cli_options.max_page, Some(999));
    assert_eq!(layer.url_find_rules.len(), 2);
    assert_eq!(layer.url_find_rules[0], "url_pattern_1");
    assert_eq!(layer.url_find_rules[1], "url_pattern_2");
    assert_eq!(layer.js_find_rules.len(), 1);
    assert_eq!(layer.js_find_rules[0], "js_pattern_1");
    let rules = layer.rules.as_ref().expect("rules");
    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].name, "CustomRule");
    assert!(rules[0].loaded);
    assert_eq!(rules[1].name, "DisabledRule");
    assert!(!rules[1].loaded);
}

#[test]
fn yaml_overrides_defaults() {
    let mut cfg = Config::default();
    let yaml = yaml_layer_from_str(
        r#"debug: true
url: "https://from-yaml.example"
max_page: 777
"#,
    );
    cfg.apply_file_layer(yaml).expect("valid");
    assert!(cfg.debug);
    assert_eq!(cfg.url, "https://from-yaml.example");
    assert_eq!(cfg.max_page, 777);
    assert_eq!(cfg.max_connections, 100);
    assert_eq!(cfg.user_agent, None);
}

#[test]
fn yaml_partial_only_overrides_specified_fields() {
    let mut cfg = Config::default();
    let yaml = yaml_layer_from_str(
        r#"debug: true
proxy: "socks5://yaml-proxy:1080"
"#,
    );
    cfg.apply_file_layer(yaml).expect("valid");
    assert!(cfg.debug);
    assert_eq!(cfg.proxy.as_deref(), Some("socks5://yaml-proxy:1080"));
    assert!(!cfg.follow_redirect);
    assert!(!cfg.hide_regex);
    assert_eq!(cfg.max_page, 100_000);
    assert_eq!(cfg.url, "");
    assert_eq!(cfg.user_agent, None);
}

#[test]
fn yaml_rule_compilation_and_appending() {
    let mut cfg = Config::default_with_rules();
    let orig_js = cfg.js_find_rules.len();
    let orig_custom = cfg.custom_rules.len();
    let yaml = yaml_layer_from_str(
        r#"jsFind:
  - "test_js_pattern\\d+"
rules:
  - name: "TestSecret"
    regex: "SECRET_[A-Z]+"
    loaded: true
"#,
    );
    cfg.apply_file_layer(yaml).expect("valid");
    assert!(cfg.js_find_rules.len() > orig_js);
    assert!(cfg.custom_rules.len() > orig_custom);
    let last = cfg.custom_rules.last().unwrap();
    assert_eq!(last.name, "TestSecret");
    assert!(last.regex.is_match("SECRET_KEY_XYZ"));
}

#[test]
fn yaml_loaded_false_rules_are_skipped() {
    let mut cfg = Config::default_with_rules();
    let orig = cfg.custom_rules.len();
    let yaml = yaml_layer_from_str(
        r#"rules:
  - name: "ActiveRule"
    regex: "active_\\d+"
    loaded: true
  - name: "SkippedRule"
    regex: "skip_me"
    loaded: false
"#,
    );
    cfg.apply_file_layer(yaml).expect("valid");
    assert_eq!(cfg.custom_rules.len(), orig + 1);
    assert_eq!(cfg.custom_rules.last().unwrap().name, "ActiveRule");
}

#[test]
fn yaml_invalid_regex_returns_error() {
    let mut cfg = Config::default();
    let yaml = yaml_layer_from_str(
        r#"rules:
  - name: "BadRule"
    regex: "[unclosed"
    loaded: true
"#,
    );
    let r = cfg.apply_file_layer(yaml);
    assert!(r.is_err());
    assert!(r.unwrap_err().to_string().contains("fail to compile regex"));
}

#[test]
fn yaml_invalid_regex_still_applies_field_overrides() {
    let mut cfg = Config::default();
    let yaml = yaml_layer_from_str(
        r#"debug: true
max_page: 555
rules:
  - name: "BadRule"
    regex: "[unclosed"
    loaded: true
"#,
    );
    let r = cfg.apply_file_layer(yaml);
    assert!(r.is_err());
    assert!(cfg.debug);
    assert_eq!(cfg.max_page, 555);
}

#[test]
fn full_cascade_cli_wins_over_yaml() {
    let mut cfg = Config::default();
    let yaml = yaml_layer_from_str(
        r#"debug: false
max_page: 300
url: "https://from-yaml.example"
proxy: "http://yaml-proxy:3128"
"#,
    );
    cfg.apply_file_layer(yaml).expect("valid");
    let cli = CliConfigLayer {
        debug: Some(true),
        max_page: Some(999),
        url: Some("https://from-cli.example".into()),
        ..Default::default()
    };
    cfg.apply_cli_layer(cli);
    assert!(cfg.debug);
    assert_eq!(cfg.max_page, 999);
    assert_eq!(cfg.url, "https://from-cli.example");
    assert_eq!(cfg.proxy.as_deref(), Some("http://yaml-proxy:3128"));
    assert_eq!(cfg.max_connections, 100);
    assert!(!cfg.follow_redirect);
}

#[test]
fn cascade_yaml_overrides_default_when_cli_unspecified() {
    let mut cfg = Config::default();
    let yaml = yaml_layer_from_str(
        r#"follow_redirect: true
max_keepalive_connections: 25
"#,
    );
    cfg.apply_file_layer(yaml).expect("valid");
    let cli = CliConfigLayer {
        url: Some("https://target.example".into()),
        ..Default::default()
    };
    cfg.apply_cli_layer(cli);
    assert!(cfg.follow_redirect);
    assert_eq!(cfg.max_keepalive_connections, 25);
    assert_eq!(cfg.url, "https://target.example");
    assert_eq!(cfg.max_connections, 100);
}

#[test]
fn cascade_preserves_rules_through_all_layers() {
    let mut cfg = Config::default_with_rules();
    let builtin_url = cfg.url_find_rules.len();
    let builtin_custom = cfg.custom_rules.len();
    let yaml = yaml_layer_from_str(
        r#"rules:
  - name: "YamlRule"
    regex: "yaml_secret"
    loaded: true
"#,
    );
    cfg.apply_file_layer(yaml).expect("valid");
    let cli = CliConfigLayer {
        debug: Some(true),
        ..Default::default()
    };
    cfg.apply_cli_layer(cli);
    assert_eq!(cfg.url_find_rules.len(), builtin_url);
    assert_eq!(cfg.custom_rules.len(), builtin_custom + 1);
    assert!(cfg.custom_rules.last().unwrap().name == "YamlRule");
    assert!(cfg.debug);
}

#[test]
fn cascade_option_field_none_does_not_clear_earlier_layer() {
    let mut cfg = Config::default();
    let yaml = yaml_layer_from_str(
        r#"user_agent: "yaml-ua"
proxy: "yaml-proxy"
"#,
    );
    cfg.apply_file_layer(yaml).expect("valid");
    assert_eq!(cfg.user_agent.as_deref(), Some("yaml-ua"));
    assert_eq!(cfg.proxy.as_deref(), Some("yaml-proxy"));
    let cli = CliConfigLayer {
        max_page: Some(42),
        ..Default::default()
    };
    cfg.apply_cli_layer(cli);
    assert_eq!(cfg.user_agent.as_deref(), Some("yaml-ua"));
    assert_eq!(cfg.proxy.as_deref(), Some("yaml-proxy"));
    assert_eq!(cfg.max_page, 42);
}

#[test]
fn validate_succeeds_with_url() {
    let mut cfg = Config::default();
    cfg.url = "https://example.com".into();
    assert!(cfg.validate().is_ok());
}

#[test]
fn validate_succeeds_with_url_file() {
    let mut cfg = Config::default();
    cfg.url_file = Some(PathBuf::from("urls.txt"));
    assert!(cfg.validate().is_ok());
}

#[test]
fn validate_succeeds_with_local() {
    let mut cfg = Config::default();
    cfg.local = Some(PathBuf::from("/tmp/scan_dir"));
    assert!(cfg.validate().is_ok());
}

#[test]
fn validate_succeeds_with_all_inputs() {
    let mut cfg = Config::default();
    cfg.url = "https://example.com".into();
    cfg.url_file = Some(PathBuf::from("urls.txt"));
    cfg.local = Some(PathBuf::from("/tmp/scan"));
    assert!(cfg.validate().is_ok());
}

#[test]
fn validate_fails_when_all_inputs_empty() {
    let mut cfg = Config::default();
    cfg.url.clear();
    cfg.url_file = None;
    cfg.local = None;
    let r = cfg.validate();
    assert!(r.is_err());
    assert!(r.unwrap_err().to_string().contains("At least one of"));
}

#[test]
fn validate_fails_with_empty_url_and_no_alternatives() {
    let mut cfg = Config::default();
    cfg.url = String::new();
    cfg.url_file = None;
    cfg.local = None;
    assert!(cfg.validate().is_err());
}

#[test]
fn rule_serialization_includes_loaded_true() {
    let rule = Rule::new("TestRule".to_string(), r"\d+").expect("valid");
    let s = serde_yaml::to_string(&rule).expect("serializable");
    assert!(s.contains("loaded: true"));
    assert!(s.contains("name: TestRule"));
    assert!(s.contains("regex:"));
}

#[test]
fn rule_new_rejects_invalid_regex() {
    assert!(Rule::new("BadRule".to_string(), "[unclosed").is_err());
}

#[test]
fn mode_from_str_normal() {
    assert!(matches!("1".parse::<Mode>().unwrap(), Mode::Normal));
    assert!(matches!(" 1 ".parse::<Mode>().unwrap(), Mode::Normal));
}

#[test]
fn mode_from_str_thorough() {
    assert!(matches!("2".parse::<Mode>().unwrap(), Mode::Thorough));
}

#[test]
fn mode_from_str_invalid() {
    assert!("3".parse::<Mode>().is_err());
    assert!("normal".parse::<Mode>().is_err());
    assert!("".parse::<Mode>().is_err());
}

#[test]
fn mode_default_is_normal() {
    assert!(matches!(Mode::default(), Mode::Normal));
}

#[test]
fn parse_status_range_exact() {
    let r = parse_status_range("200").expect("valid");
    assert_eq!(r.len(), 1);
    assert!(matches!(r[0], StatusRange::Exact(200)));
}

#[test]
fn parse_status_range_multiple_exact() {
    let r = parse_status_range("200,404,500").expect("valid");
    assert_eq!(r.len(), 3);
    assert!(matches!(r[0], StatusRange::Exact(200)));
    assert!(matches!(r[1], StatusRange::Exact(404)));
    assert!(matches!(r[2], StatusRange::Exact(500)));
}

#[test]
fn parse_status_range_with_ranges() {
    let r = parse_status_range("200-299,404").expect("valid");
    assert_eq!(r.len(), 2);
    assert!(matches!(r[0], StatusRange::Range(200, 299)));
    assert!(matches!(r[1], StatusRange::Exact(404)));
}

#[test]
fn status_range_rule_allows_exact_and_range_matches() {
    let rule = StatusRangeRule::from(vec![StatusRange::Exact(201), StatusRange::Range(300, 399)]);

    assert!(rule.is_allowed(201));
    assert!(rule.is_allowed(300));
    assert!(rule.is_allowed(399));
    assert!(!rule.is_allowed(200));
    assert!(!rule.is_allowed(400));
}

#[test]
fn parse_status_range_invalid() {
    assert!(parse_status_range("abc").is_err());
    assert!(parse_status_range("200-abc").is_err());
    assert!(parse_status_range("").is_err());
}

#[test]
fn parse_domain_filter_basic() {
    let r = parse_domain_filter("example.com,*.gov, test.io ").expect("valid");
    assert_eq!(r, vec!["example.com", "*.gov", "test.io"]);
}

#[test]
fn parse_domain_filter_empty() {
    assert!(parse_domain_filter("").expect("valid").is_empty());
}

#[test]
fn parse_domain_filter_whitespace_only() {
    assert!(parse_domain_filter("  ,  ,  ").expect("valid").is_empty());
}
