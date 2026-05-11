//! CLI, YAML, and runtime configuration types.

use std::{
    collections::BTreeMap,
    error,
    fs::File,
    io::{self, BufReader},
    path::PathBuf,
    str::FromStr,
    time::Duration,
};

use anyhow::{Result, anyhow, bail};
use clap::{ArgAction, Parser, ValueEnum};
use pub_fields::pub_fields;
use regex::Regex;
use reqwest::header::{ACCEPT, HeaderMap, HeaderName, HeaderValue, USER_AGENT};
use serde::{
    Deserialize, Serialize, de::DeserializeOwned, ser::SerializeMap, ser::SerializeStruct,
};

/// HTTP status filter entry.
#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub enum StatusRange {
    /// Match one exact status code.
    Exact(u16),
    /// Match an inclusive status-code range.
    Range(u16, u16),
}
/// Set of accepted HTTP status codes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusRangeRule {
    ranges: Vec<StatusRange>,
}
impl StatusRangeRule {
    /// Check if the given status matches any of the allowed ranges.
    pub fn is_allowed(&self, status_code: u16) -> bool {
        self.ranges.iter().any(|range| match range {
            StatusRange::Exact(s) => *s == status_code,
            StatusRange::Range(start, end) => *start <= status_code && status_code <= *end,
        })
    }
}
impl From<Vec<StatusRange>> for StatusRangeRule {
    fn from(ranges: Vec<StatusRange>) -> Self {
        Self { ranges }
    }
}

/// CLI argument layer — every field is optional so it can be merged
/// over a base [`Config`]. Applied after any YAML config layer.
#[pub_fields]
#[derive(Debug, Default, Deserialize, Serialize, Parser)]
#[command(version, about)]
pub struct CliConfigLayer {
    #[arg(long, action = ArgAction::SetTrue, help = "Enable debug")]
    /// Enable debug logging.
    pub debug: Option<bool>,

    #[arg(short = 'a', long = "ua", help = "Set User-Agent")]
    /// User-Agent header override.
    pub user_agent: Option<String>,

    #[arg(short = 'c', long, help = "Set cookie")]
    /// Cookie header value.
    pub cookie: Option<String>,

    #[arg(
        short = 'd',
        long,
        help = "Domain white list, wildcard(*) is supported, separated by commas, e.g. *.example.com, example*",
        value_parser = parse_domain_filter,
    )]
    /// Domain allow-list patterns.
    pub allow_domains: Option<Vec<String>>,

    #[arg(
        short = 'D',
        long,
        help = "Domain black list, wildcard(*) is supported, separated by commas, e.g. *.example.com, example*",
        value_parser = parse_domain_filter,
    )]
    /// Domain block-list patterns.
    pub disallow_domains: Option<Vec<String>>,

    #[arg(short = 'f', long,
        help = "Target urls file, separated by line break",
        value_parser = existing_file)]
    /// File containing newline-delimited seed URLs.
    pub url_file: Option<PathBuf>,

    #[arg(
        short = 'i',
        long,
        help = "Set config file, defaults to setting.yaml",
        default_value = "setting.yaml"
    )]
    #[serde(default = "default_config_path")]
    /// YAML configuration file path.
    pub config: PathBuf,

    #[arg(
        short = 'm',
        long,
        help = "Set crawl mode, 'normal' for max_depth=1, 'thorough' for max_depth=2, default to 'normal'",
        value_enum
    )]
    /// Crawl mode preset.
    pub mode: Option<Mode>,

    #[arg(long, help = "Max page number to crawl")]
    /// Maximum pages to crawl.
    pub max_page: Option<u32>,

    #[arg(long, help = "Max depth to crawl, 0 means only crawl the seed urls")]
    /// Maximum crawl depth.
    pub max_depth: Option<u32>,

    #[arg(long, help = "Max keep-alive HTTP connections per domain")]
    /// Maximum concurrent requests per domain.
    pub max_concurrency_per_domain: Option<usize>,

    #[arg(long, help = "Minimum seconds between requests to the same domain")]
    /// Minimum seconds between requests to the same domain.
    pub min_request_interval: Option<f32>,

    #[arg(
        long,
        short = 'o',
        help = "Output result to specified file in csv format"
    )]
    /// Output file path.
    pub outfile: Option<PathBuf>,

    #[arg(
        short,
        long = "status",
        help = "Filter response status to display, seperated by commas, e.g. 200,300-400",
        value_parser = parse_status_range,
    )]
    /// Response status display filter.
    pub status_filter: Option<StatusRangeRule>,

    #[arg(
        short = 'x',
        long,
        help = "Set proxy, e.g. http://127.0.0.1:8080, socks5://127.0.0.1:7890"
    )]
    /// Proxy URL.
    pub proxy: Option<String>,

    #[arg(
        short = 'H',
        long,
        action = ArgAction::SetTrue,
        help = "Hide regex search result"
    )]
    /// Hide regex/secret output.
    pub hide_regex: Option<bool>,

    #[arg(short = 'F', long, action = ArgAction::SetTrue, help = "Follow redirects")]
    /// Follow HTTP redirects.
    pub follow_redirect: Option<bool>,

    #[arg(short, long, help = "Target URL")]
    /// Single crawl seed URL.
    pub url: Option<String>,

    #[arg(long, action = ArgAction::SetTrue, help = "Show detailed result")]
    /// Show detailed output.
    pub detail: Option<bool>,

    #[arg(
        long,
        action = ArgAction::SetTrue,
        help = "Validate the status of found urls"
    )]
    /// Validate discovered link statuses.
    pub validate: Option<bool>,

    #[arg(
        short,
        long,
        help = "Local file or directory, scan local file/directory recursively",
        value_parser = existing_file,
    )]
    /// Local file or directory to scan.
    pub local: Option<PathBuf>,
}
/// Trait for loading typed configuration from YAML files.
pub trait LoadFromYaml<T: DeserializeOwned> {
    /// Load and deserialize YAML from `path`.
    fn load_from_yaml(path: PathBuf) -> Result<T, Box<dyn error::Error>> {
        if !path.is_file() {
            return Err(io::Error::other(format!("{} is not a yaml file", path.display())).into());
        }
        let f = File::open(path)?;
        let cfg: T = serde_yaml::from_reader(BufReader::new(f))?;
        Ok(cfg)
    }
}

impl LoadFromYaml<CliConfigLayer> for CliConfigLayer {}

/// YAML configuration layer.
#[derive(Deserialize, Debug)]
pub struct FileConfigLayer {
    #[serde(flatten)]
    /// CLI-shaped options embedded in YAML.
    pub cli_options: CliConfigLayer,

    /// Request timeout in seconds.
    pub timeout: Option<f32>,
    /// Maximum concurrent requests per domain.
    pub max_concurrent_per_domain: Option<usize>,
    /// Whether crawler follows redirects.
    pub follow_redirects: Option<bool>,
    /// Maximum number of pages to crawl.
    pub max_page_num: Option<u32>,
    #[serde(rename = "dangerousPath")]
    /// Dangerous path fragments to avoid.
    pub dangerous_paths: Option<Vec<String>>,
    #[serde(
        rename = "headers",
        deserialize_with = "deserialize_optional_headers",
        default
    )]
    /// Custom HTTP headers.
    pub custom_headers: Option<HeaderMap>,

    #[serde(rename = "urlFind")]
    /// Extra regex patterns for URL discovery.
    pub url_find_rules: Vec<String>,
    #[serde(rename = "jsFind")]
    /// Extra regex patterns for JavaScript URL discovery.
    pub js_find_rules: Vec<String>,
    /// Custom secret-detection rules.
    pub rules: Option<Vec<RuleItem>>,
}
/// YAML representation of a custom regex rule.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RuleItem {
    /// Rule name displayed with matches.
    pub name: String,
    /// Regex pattern string.
    pub regex: String,
    /// Whether this rule should be loaded.
    pub loaded: bool,
}
impl LoadFromYaml<FileConfigLayer> for FileConfigLayer {}

/// Concrete runtime config built by merging layers.
/// Start with [`Config::default()`], apply YAML via [`FileConfigLayer`],
/// then apply CLI via [`CliConfigLayer`], and call [`Config::validate`].
pub struct Config {
    /// Enable debug logging.
    pub debug: bool,
    /// User-Agent header override.
    pub user_agent: Option<String>,
    /// Cookie header value.
    pub cookie: Option<String>,
    /// Domain allow-list patterns.
    pub allow_domains: Option<Vec<String>>,
    /// Domain block-list patterns.
    pub disallow_domains: Option<Vec<String>>,
    /// Newline-delimited seed URL file.
    pub url_file: Option<PathBuf>,
    /// YAML configuration path.
    pub config: PathBuf,
    /// Request timeout.
    pub timeout: Duration,
    /// Crawl mode preset.
    pub mode: Mode,
    /// Maximum pages to crawl.
    pub max_page: Option<u32>,
    /// Maximum crawl depth.
    pub max_depth: Option<u32>,
    /// Maximum concurrent requests per domain.
    pub max_concurrency_per_domain: usize,
    /// Minimum interval between requests to the same domain.
    pub min_request_interval: Duration,
    /// Output file path.
    pub outfile: Option<PathBuf>,
    /// Response status display filter.
    pub status_filter: Option<StatusRangeRule>,
    /// Proxy URL.
    pub proxy: Option<String>,
    /// Hide regex/secret output.
    pub hide_regex: bool,
    /// Follow HTTP redirects.
    pub follow_redirect: bool,
    /// Dangerous path fragments to avoid.
    pub dangerous_paths: Option<Vec<String>>,
    /// Single seed URL.
    pub url: Option<String>,
    /// Show detailed output.
    pub detail: bool,
    /// Validate discovered link statuses.
    pub validate: bool,
    /// Local file or directory to scan.
    pub local: Option<PathBuf>,
    /// URL discovery regex rules.
    pub url_find_rules: Vec<Rule>,
    /// JavaScript URL discovery regex rules.
    pub js_find_rules: Vec<Rule>,
    /// Secret-detection regex rules.
    pub custom_rules: Vec<Rule>,
    /// Custom HTTP headers.
    pub custom_headers: Option<HeaderMap>,
}
impl Serialize for Config {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut config = serializer.serialize_struct("Config", 29)?;
        config.serialize_field("debug", &self.debug)?;
        config.serialize_field("user_agent", &self.user_agent)?;
        config.serialize_field("cookie", &self.cookie)?;
        config.serialize_field("allow_domains", &self.allow_domains)?;
        config.serialize_field("disallow_domains", &self.disallow_domains)?;
        config.serialize_field("url_file", &self.url_file)?;
        config.serialize_field("config", &self.config)?;
        config.serialize_field("timeout", &self.timeout.as_secs_f32())?;
        config.serialize_field("mode", &self.mode)?;
        config.serialize_field("max_page", &self.max_page)?;
        config.serialize_field("max_depth", &self.max_depth)?;
        config.serialize_field(
            "max_concurrent_per_domain",
            &self.max_concurrency_per_domain,
        )?;
        config.serialize_field(
            "min_request_interval",
            &self.min_request_interval.as_secs_f32(),
        )?;
        config.serialize_field("outfile", &self.outfile)?;
        config.serialize_field("status_filter", &self.status_filter)?;
        config.serialize_field("proxy", &self.proxy)?;
        config.serialize_field("hide_regex", &self.hide_regex)?;
        config.serialize_field("follow_redirects", &self.follow_redirect)?;
        config.serialize_field("dangerousPath", &self.dangerous_paths)?;
        config.serialize_field("url", &self.url)?;
        config.serialize_field("detail", &self.detail)?;
        config.serialize_field("validate", &self.validate)?;
        config.serialize_field("local", &self.local)?;
        let url_find_rules = &self
            .url_find_rules
            .iter()
            .map(|r| r.regex.to_string())
            .collect::<Vec<String>>();
        config.serialize_field("urlFind", url_find_rules)?;
        config.serialize_field(
            "jsFind",
            &self
                .js_find_rules
                .iter()
                .map(|r| r.regex.to_string())
                .collect::<Vec<String>>(),
        )?;
        config.serialize_field("rules", &self.custom_rules)?;
        config.serialize_field("headers", &serializable_headers(&self.custom_headers))?;
        config.end()
    }
}
/// Compiled regex rule.
pub struct Rule {
    /// Rule name displayed with matches.
    pub name: String,
    /// Compiled regex pattern.
    pub regex: Regex,
}
impl Rule {
    /// Compile a new named regex rule.
    pub fn new(name: String, regex: &str) -> Result<Self, regex::Error> {
        Ok(Self {
            name,
            regex: Regex::new(regex)?,
        })
    }
}
impl Config {
    fn default_url_find_rules() -> Vec<Rule> {
        vec![
                            Rule::new(
                "builtin_1".to_string(),
                r#"["'‘“`]\s{0,6}(https{0,1}:[-a-zA-Z0-9()@:%_\+.~#?&//={}]{2,100}?)\s{0,6}["''‘“`]'"#,
            ).unwrap(),
                Rule::new(
                "builtin_2".to_string(),
                r#"=\s{0,6}(https{0,1}:[-a-zA-Z0-9()@:%_\+.~#?&//={}]{2,100})"#,
            ).unwrap(),
                Rule::new(
                "builtin_3".to_string(),
                r#"["'‘“`]\s{0,6}([#,.]{0,2}/[-a-zA-Z0-9()@:%_\+.~#?&//={}]{2,100}?)\s{0,6}["''‘“`]"#,
            ).unwrap(),
                Rule::new(
                "builtin_4".to_string(),
                r#""([-a-zA-Z0-9()@:%_\+.~#?&//={}]+?[/]{1}[-a-zA-Z0-9()@:%_\+.~#?&//={}]+?)""#,
            ).unwrap(),
                Rule::new(
                "builtin_5".to_string(),
                r#"href\s{0,6}=\s{0,6}["'‘“`]{0,1}\s{0,6}([-a-zA-Z0-9()@:%_\+.~#?&//={}]{2,100})|action\s{0,6}=\s{0,6}["'‘“`]{0,1}\s{0,6}([-a-zA-Z0-9()@:%_\+.~#?&//={}]{2,100})"#,
            ).unwrap(),


        ]
    }
    fn default_js_find_rules() -> Vec<Rule> {
        vec![
                        Rule::new(
                "builtin_6".to_string(),
                r#"(https{0,1}:[-a-zA-Z0-9（）@:%_\+.~#?&//=]{2,100}?[-a-zA-Z0-9（）@:%_\+.~#?&//=]{3}[.]js)"#,
            ).unwrap(),
                Rule::new(
                "builtin_7".to_string(),
                r#"["'‘“`]\s{0,6}(/{0,1}[-a-zA-Z0-9（）@:%_\+.~#?&//=]{2,100}?[-a-zA-Z0-9（）@:%_\+.~#?&//=]{3}[.]js)"#,
            ).unwrap(),
                Rule::new(
                "builtin_8".to_string(),
                r#"=\s{0,6}[",',’,”]{0,1}\s{0,6}(/{0,1}[-a-zA-Z0-9（）@:%_\+.~#?&//=]{2,100}?[-a-zA-Z0-9（）@:%_\+.~#?&//=]{3}[.]js)"#,
            ).unwrap(),
        ]
    }
    fn default_custom_rules() -> Vec<Rule> {
        vec![
                            Rule::new(
                "Swagger".to_string(),
                r#"\b[\w/]+?((swagger-ui.html)|(\"swagger\":)|(Swagger UI)|(swaggerUi)|(swaggerVersion))\b"#,
            ).unwrap(),
                Rule::new(
                "ID Card".to_string(),
                r#"\b((\d{8}(0\d|10|11|12)([0-2]\d|30|31)\d{3}\$)|(\d{6}(18|19|20)\d{2}(0[1-9]|10|11|12)([0-2]\d|30|31)\d{3}(\d|X|x)))\b"#,
            ).unwrap(),
                Rule::new(
                "Phone".to_string(),
                r#"\b((?:(?:\+|00)86)?1(?:(?:3[\d])|(?:4[5-79])|(?:5[0-35-9])|(?:6[5-7])|(?:7[0-8])|(?:8[\d])|(?:9[189]))\d{8})\b"#,
            ).unwrap(),
                Rule::new(
                "JS Map".to_string(),
                r#"\b([\w/]+?\.js\.map)"#,
            ).unwrap(),
                Rule::new(
                "URL as a value".to_string(),
                r#"(\b\w+?=(https?)(://|%3a%2f%2f))"#,
            ).unwrap(),
                Rule::new(
                "Email".to_string(),
                r#"\b(([a-z0-9][_|\.])*[a-z0-9]+@([a-z0-9][-|_|\.])*[a-z0-9]+\.([a-z]{2,}))\b"#,
            ).unwrap(),
                Rule::new(
                "Internal IP".to_string(),
                r#"[^0-9]((127\.0\.0\.1)|(10\.\d{1,3}\.\d{1,3}\.\d{1,3})|(172\.((1[6-9])|(2\d)|(3[01]))\.\d{1,3}\.\d{1,3})|(192\.168\.\d{1,3}\.\d{1,3}))"#,
            ).unwrap(),
                Rule::new(
                "Cloud Key".to_string(),
                r#"\b((accesskeyid)|(accesskeysecret)|\b(LTAI[a-z0-9]{12,20}))\b"#,
            ).unwrap(),
                Rule::new(
                "Shiro".to_string(),
                r#"(=deleteMe|rememberMe=)"#,
            ).unwrap(),
                Rule::new(
                "Suspicious API Key".to_string(),
                r#"["'][0-9a-zA-Z]{32}['"]"#,
            ).unwrap(),

        ]
    }
    /// Default [`Config`] with built-in rules filled.
    pub fn default_with_rules() -> Self {
        Self {
            url_find_rules: Self::default_url_find_rules(),
            js_find_rules: Self::default_js_find_rules(),
            custom_rules: Self::default_custom_rules(),
            ..Self::default()
        }
    }
}

impl Default for Config {
    /// Default [`Config`] without any rule
    fn default() -> Self {
        Self {
            debug: false,
            user_agent: None,
            cookie: None,
            allow_domains: None,
            disallow_domains: None,
            url_file: None,
            config: default_config_path(),
            mode: Mode::Normal,
            max_page: Some(100000),
            timeout: Duration::from_secs(30),
            dangerous_paths: None,
            max_depth: None,
            custom_headers: Some(default_headers()),
            max_concurrency_per_domain: 50,
            min_request_interval: Duration::from_millis(200),
            outfile: None,
            status_filter: None,
            proxy: None,
            hide_regex: false,
            follow_redirect: false,
            url: None,
            detail: false,
            validate: false,
            local: None,
            url_find_rules: vec![],
            js_find_rules: vec![],
            custom_rules: vec![],
        }
    }
}
impl Serialize for Rule {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut rule = serializer.serialize_map(Some(3))?;
        rule.serialize_entry("name", &self.name)?;
        rule.serialize_entry("regex", &self.regex.to_string())?;
        rule.serialize_entry("loaded", &true)?;
        rule.end()
    }
}

impl Config {
    /// Merge a [`CliConfigLayer`] into this config. Only `Some` fields in the
    /// layer override the current values — `None` fields are skipped.
    pub fn apply_cli_layer(&mut self, layer: CliConfigLayer) {
        if let Some(v) = layer.debug {
            self.debug = v;
        }
        if let Some(v) = layer.user_agent {
            self.user_agent = Some(v);
        }
        if let Some(v) = layer.cookie {
            self.cookie = Some(v);
        }
        if let Some(v) = layer.allow_domains {
            self.allow_domains = Some(v);
        }
        if let Some(v) = layer.disallow_domains {
            self.disallow_domains = Some(v);
        }
        if let Some(v) = layer.url_file {
            self.url_file = Some(v);
        }
        self.config = layer.config;
        if let Some(v) = layer.mode {
            self.mode = v;
        }
        if let Some(v) = layer.max_page {
            self.max_page = Some(v);
        }
        if let Some(v) = layer.max_depth {
            self.max_depth = Some(v);
        }
        if let Some(v) = layer.max_concurrency_per_domain {
            self.max_concurrency_per_domain = v;
        }
        if let Some(v) = layer.min_request_interval {
            self.min_request_interval = Duration::from_secs_f32(v);
        }
        if let Some(v) = layer.outfile {
            self.outfile = Some(v);
        }
        if let Some(v) = layer.status_filter {
            self.status_filter = Some(v);
        }
        if let Some(v) = layer.proxy {
            self.proxy = Some(v);
        }
        if let Some(v) = layer.hide_regex {
            self.hide_regex = v;
        }
        if let Some(v) = layer.follow_redirect {
            self.follow_redirect = v;
        }
        if let Some(v) = layer.url {
            self.url = Some(v);
        }
        if let Some(v) = layer.detail {
            self.detail = v;
        }
        if let Some(v) = layer.validate {
            self.validate = v;
        }
        if let Some(v) = layer.local {
            self.local = Some(v);
        }
    }
    /// Merge a YAML file layer into this config.
    pub fn apply_file_layer(&mut self, layer: FileConfigLayer) -> Result<()> {
        if let Some(v) = layer.timeout {
            self.timeout = Duration::from_secs_f32(v);
        }
        if let Some(v) = layer.max_concurrent_per_domain {
            self.max_concurrency_per_domain = v;
        }
        if let Some(v) = layer.follow_redirects {
            self.follow_redirect = v;
        }
        if let Some(v) = layer.max_page_num {
            self.max_page = Some(v);
        }
        if let Some(v) = layer.dangerous_paths {
            self.dangerous_paths = Some(v);
        }
        if let Some(v) = layer.custom_headers {
            self.custom_headers
                .get_or_insert_with(HeaderMap::new)
                .extend(v);
        }
        self.apply_cli_layer(layer.cli_options);
        let mut errors = vec![];
        let mut add_rules = |rules: Vec<String>, name_prefix| {
            for (i, s) in rules.iter().enumerate() {
                match Rule::new(format!("{name_prefix}_{i}"), s) {
                    Ok(r) => {
                        self.js_find_rules.push(r);
                    }
                    Err(e) => {
                        errors.push(e);
                    }
                }
            }
        };
        add_rules(layer.js_find_rules, "jsFind");
        add_rules(layer.url_find_rules, "urlFind");
        if let Some(r) = layer.rules {
            for item in r.iter().filter(|i| i.loaded) {
                match Rule::new(item.name.clone(), &item.regex) {
                    Ok(rule) => {
                        self.custom_rules.push(rule);
                    }
                    Err(e) => {
                        errors.push(e);
                    }
                }
            }
        }

        if !errors.is_empty() {
            return Err(anyhow!(
                "fail to compile regex:\n {}",
                errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<String>>()
                    .join("\n")
            ));
        }
        Ok(())
    }

    /// Validate that required fields are present.
    pub fn validate(&self) -> Result<()> {
        let has_url = self.url.as_ref().is_some_and(|url| !url.is_empty());
        if !has_url && self.url_file.is_none() && self.local.is_none() {
            bail!("At least one of --url, --url-file, or --local must be specified".to_string(),);
        }
        Ok(())
    }
}

/// Crawl mode preset.
#[derive(Debug, ValueEnum, Clone, Serialize, Deserialize, Default)]
pub enum Mode {
    #[default]
    /// Normal mode uses a max-depth preset of 1.
    Normal,
    /// Thorough mode uses a max-depth preset of 2.
    Thorough,
}

impl FromStr for Mode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        match s {
            "1" => Ok(Mode::Normal),
            "2" => Ok(Mode::Thorough),
            _ => Err(format!("invalid mode: {}", s)),
        }
    }
}

/// Parse a comma-separated domain filter list.
pub fn parse_domain_filter(s: &str) -> Result<Vec<String>> {
    s.split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(|e| Ok(e.to_owned()))
        .collect()
}

fn deserialize_optional_headers<'de, D>(
    deserializer: D,
) -> core::result::Result<Option<HeaderMap>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<BTreeMap<String, String>>::deserialize(deserializer)?;
    raw.map(headers_from_map)
        .transpose()
        .map_err(serde::de::Error::custom)
}

fn serializable_headers(headers: &Option<HeaderMap>) -> Option<BTreeMap<String, String>> {
    headers.as_ref().map(|headers| {
        headers
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_owned(), value.to_owned()))
            })
            .collect()
    })
}

fn default_config_path() -> PathBuf {
    PathBuf::from("setting.yaml")
}

fn default_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT,HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/80.0.3987.87 Safari/537.36 SE 2.X MetaSr 1.0"));
    headers.insert(ACCEPT, HeaderValue::from_static("*/*"));
    headers
}

fn headers_from_map(raw: BTreeMap<String, String>) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    for (name, value) in raw {
        let header_name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|e| anyhow!("invalid header name '{name}': {e}"))?;
        let header_value = HeaderValue::from_str(&value)
            .map_err(|e| anyhow!("invalid header value for '{name}': {e}"))?;
        headers.insert(header_name, header_value);
    }
    Ok(headers)
}

fn existing_file(s: &str) -> Result<PathBuf> {
    let p = PathBuf::from(s);
    match p.exists() {
        true => Ok(p),
        false => Err(anyhow!("file does not exist: {}", s)),
    }
}

/// Parse comma-separated exact HTTP status codes and inclusive ranges.
pub fn parse_status_range(s: &str) -> Result<Vec<StatusRange>> {
    s.split(',')
        .map(|e: &str| -> Result<StatusRange> {
            let mut parts = e.splitn(2, '-').map(str::trim);
            let start: u16 = parts
                .next()
                .ok_or_else(|| anyhow!("invalid status range: '{e}'"))?
                .parse()
                .map_err(|err| anyhow!("invalid status range: '{e}' {err}"))?;
            match parts.next() {
                Some(end) => {
                    let end: u16 = end
                        .parse()
                        .map_err(|err| anyhow!("invalid status range: '{e}' {err}"))?;
                    Ok(StatusRange::Range(start, end))
                }
                None => Ok(StatusRange::Exact(start)),
            }
        })
        .collect()
}

#[test]
fn verify_cli() {
    use clap::CommandFactory;
    CliConfigLayer::command().debug_assert();
}

#[test]
fn verify_help() {
    let result = CliConfigLayer::try_parse_from(["secret-scraper", "--help"]);
    // --help triggers a DisplayHelp error in clap, which is expected
    match result {
        Ok(_) => panic!("--help should exit with DisplayHelp"),
        Err(e) => match e.kind() {
            clap::error::ErrorKind::DisplayHelp => { /* expected */ }
            _ => panic!("unexpected error: {e}"),
        },
    }
}
