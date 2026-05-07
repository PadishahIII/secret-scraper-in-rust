use std::{
    error,
    fs::File,
    io::{self, BufReader},
    path::PathBuf,
    str::FromStr,
};

use anyhow::{Result, anyhow, bail};
use clap::{Parser, ValueEnum};
use pub_fields::pub_fields;
use regex::Regex;
use serde::{Deserialize, Serialize, de::DeserializeOwned, ser::SerializeMap};

#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub enum StatusRange {
    Exact(u16),
    Range(u16, u16),
}
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
    #[arg(long, help = "Enable debug")]
    pub debug: Option<bool>,

    #[arg(short = 'a', long = "ua", help = "Set User-Agent")]
    pub user_agent: Option<String>,

    #[arg(short = 'c', long, help = "Set cookie")]
    pub cookie: Option<String>,

    #[arg(
        short = 'd',
        long,
        help = "Domain white list, wildcard(*) is supported, separated by commas, e.g. *.example.com, example*",
        value_parser = parse_domain_filter,
    )]
    pub allow_domains: Option<Vec<String>>,

    #[arg(
        short = 'D',
        long,
        help = "Domain black list, wildcard(*) is supported, separated by commas, e.g. *.example.com, example*",
        value_parser = parse_domain_filter,
    )]
    pub disallow_domains: Option<Vec<String>>,

    #[arg(short = 'f', long,
        help = "Target urls file, separated by line break",
        value_parser = existing_file)]
    pub url_file: Option<PathBuf>,

    #[arg(short = 'i', long, help = "Set config file, defaults to settings.yml", value_parser = existing_file)]
    pub config: Option<PathBuf>,

    #[arg(
        short = 'm',
        long,
        help = "Set crawl mode, '1' for max_depth=1, '2' for max_depth=2",
        value_enum
    )]
    pub mode: Option<Mode>,

    #[arg(long, help = "Max page number to crawl")]
    pub max_page: Option<u32>,

    #[arg(long, help = "Max total HTTP connections")]
    pub max_connections: Option<usize>,

    #[arg(long, help = "Max keep-alive HTTP connections")]
    pub max_keepalive_connections: Option<usize>,

    #[arg(long, help = "Max keep-alive HTTP connections per domain")]
    pub max_concurrency_per_domain: Option<usize>,

    #[arg(long, help = "Minimum seconds between requests to the same domain")]
    pub min_request_interval: Option<f32>,

    #[arg(
        long,
        short = 'o',
        help = "Output result to specified file in csv format"
    )]
    pub outfile: Option<PathBuf>,

    #[arg(
        short,
        long = "status",
        help = "Filter response status to display, seperated by commas, e.g. 200,300-400",
        value_parser = parse_status_range,
    )]
    pub status_filter: Option<StatusRangeRule>,

    #[arg(
        short = 'x',
        long,
        help = "Set proxy, e.g. http://127.0.0.1:8080, socks5://127.0.0.1:7890"
    )]
    pub proxy: Option<String>,

    #[arg(short = 'H', long, help = "Hide regex search result")]
    pub hide_regex: Option<bool>,

    #[arg(short = 'F', long, help = "Follow redirects")]
    pub follow_redirect: Option<bool>,

    #[arg(short, long, help = "Target URL")]
    pub url: Option<String>,

    #[arg(long, help = "Show detailed result")]
    pub detail: Option<bool>,

    #[arg(long, help = "Validate the status of found urls")]
    pub validate: Option<bool>,

    #[arg(
        short,
        long,
        help = "Local file or directory, scan local file/directory recursively",
        value_parser = existing_file,
    )]
    pub local: Option<PathBuf>,
}
pub trait LoadFromYaml<T: DeserializeOwned> {
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

#[derive(Deserialize, Serialize, Debug)]
pub struct FileConfigLayer {
    #[serde(flatten)]
    pub cli_options: CliConfigLayer,

    #[serde(rename = "urlFind")]
    pub url_find_rules: Vec<String>,
    #[serde(rename = "jsFind")]
    pub js_find_rules: Vec<String>,
    pub rules: Option<Vec<RuleItem>>,
}
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RuleItem {
    pub name: String,
    pub regex: String,
    pub loaded: bool,
}
impl LoadFromYaml<FileConfigLayer> for FileConfigLayer {}

/// Concrete runtime config built by merging layers.
/// Start with [`Config::default()`], apply YAML via [`ConfigLayer`],
/// then apply CLI via [`ConfigLayer`], and call [`Config::validate`].
#[derive(Serialize)]
pub struct Config {
    pub debug: bool,
    pub user_agent: Option<String>,
    pub cookie: Option<String>,
    pub allow_domains: Option<Vec<String>>,
    pub disallow_domains: Option<Vec<String>>,
    pub url_file: Option<PathBuf>,
    pub config: Option<PathBuf>,
    pub mode: Mode,
    pub max_page: u32,
    pub max_connections: usize,
    pub max_keepalive_connections: usize,
    pub max_concurrency_per_domain: Option<usize>,
    pub min_request_interval: Option<f32>,
    pub outfile: Option<PathBuf>,
    pub status_filter: Option<StatusRangeRule>,
    pub proxy: Option<String>,
    pub hide_regex: bool,
    pub follow_redirect: bool,
    pub url: String,
    pub detail: bool,
    pub validate: bool,
    pub local: Option<PathBuf>,
    #[serde(rename = "urlFind")]
    pub url_find_rules: Vec<Rule>,
    #[serde(rename = "jsFind")]
    pub js_find_rules: Vec<Rule>,
    #[serde(rename = "rules")]
    pub custom_rules: Vec<Rule>,
}
pub struct Rule {
    pub name: String,
    pub regex: Regex,
}
impl Rule {
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
    /// Defalt [`Config`] with default rules filled
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
            config: None,
            mode: Mode::Normal,
            max_page: 100000,
            max_connections: 100,
            max_keepalive_connections: 50,
            max_concurrency_per_domain: None,
            min_request_interval: None,
            outfile: None,
            status_filter: None,
            proxy: None,
            hide_regex: false,
            follow_redirect: false,
            url: String::new(),
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
    /// Merge a [`ConfigLayer`] into this config. Only `Some` fields in the
    /// layer override the current values — `None` fields are skipped.
    pub fn apply_cli_layer(&mut self, layer: CliConfigLayer) {
        macro_rules! set_value {
            ($($field:ident),* ) => {
                $(
                    if let Some(v) = layer.$field{
                        self.$field = v;
                    }
                )*

            };
        }
        macro_rules! set_option {
            ($($field:ident),* ) => {
                $(
                    if let Some(v) = layer.$field{
                        self.$field = Some(v);
                    }
                )*

            };
        }
        set_value!(
            debug,
            mode,
            max_page,
            max_connections,
            max_keepalive_connections,
            hide_regex,
            follow_redirect,
            detail,
            validate,
            url
        );
        set_option!(
            user_agent,
            cookie,
            allow_domains,
            disallow_domains,
            url_file,
            config,
            max_concurrency_per_domain,
            min_request_interval,
            outfile,
            status_filter,
            proxy,
            local
        );
    }
    pub fn apply_file_layer(&mut self, layer: FileConfigLayer) -> Result<()> {
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
        if self.url.is_empty() && self.url_file.is_none() && self.local.is_none() {
            bail!("At least one of --url, --url-file, or --local must be specified".to_string(),);
        }
        Ok(())
    }
}

#[derive(Debug, ValueEnum, Clone, Serialize, Deserialize, Default)]
pub enum Mode {
    #[value(name = "1")]
    #[default]
    Normal,
    #[value(name = "2")]
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

pub fn parse_domain_filter(s: &str) -> Result<Vec<String>> {
    s.split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(|e| Ok(e.to_owned()))
        .collect()
}

fn existing_file(s: &str) -> Result<PathBuf> {
    let p = PathBuf::from(s);
    match p.exists() {
        true => Ok(p),
        false => Err(anyhow!("file does not exist: {}", s)),
    }
}

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
