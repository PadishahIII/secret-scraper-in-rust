use std::{
    error,
    fs::File,
    io::{self, BufReader},
    path::PathBuf,
    str::FromStr,
};

use clap::{Parser, ValueEnum};
use pub_fields::pub_fields;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Copy, Serialize, Deserialize)]
pub(crate) enum StatusRange {
    Exact(u16),
    Range(u16, u16),
}

/// CLI argument layer — every field is optional so it can be merged
/// over a base [`Config`]. Applied after any YAML config layer.
#[pub_fields]
#[derive(Debug, Default, Deserialize, Parser)]
#[command(version, about)]
pub struct ConfigLayer {
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
    pub status_filter: Option<Vec<StatusRange>>,

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

impl ConfigLayer {
    pub fn load_from_yaml(path: PathBuf) -> Result<ConfigLayer, Box<dyn error::Error>> {
        if !path.is_file() {
            return Err(io::Error::other(format!("{} is not a yaml file", path.display())).into());
        }
        let f = File::open(path)?;
        let cfg: ConfigLayer = serde_yaml::from_reader(BufReader::new(f))?;
        Ok(cfg)
    }
}

/// Concrete runtime config built by merging layers.
/// Start with [`Config::default()`], apply YAML via [`ConfigLayer`],
/// then apply CLI via [`ConfigLayer`], and call [`Config::validate`].
#[pub_fields]
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub status_filter: Option<Vec<StatusRange>>,
    pub proxy: Option<String>,
    pub hide_regex: bool,
    pub follow_redirect: bool,
    pub url: String,
    pub detail: bool,
    pub validate: bool,
    pub local: Option<PathBuf>,
}

impl Default for Config {
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
        }
    }
}

impl Config {
    /// Merge a [`ConfigLayer`] into this config. Only `Some` fields in the
    /// layer override the current values — `None` fields are skipped.
    pub fn apply(&mut self, layer: ConfigLayer) {
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

    /// Validate that required fields are present.
    pub fn validate(&self) -> Result<(), String> {
        if self.url.is_empty() && self.url_file.is_none() && self.local.is_none() {
            return Err(
                "At least one of --url, --url-file, or --local must be specified".to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, ValueEnum, Clone, Serialize, Deserialize, Default)]
pub(crate) enum Mode {
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

fn parse_domain_filter(s: &str) -> Result<Vec<String>, String> {
    s.split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(|e| Ok(e.to_owned()))
        .collect()
}

fn existing_file(s: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(s);
    match p.exists() {
        true => Ok(p),
        false => Err(format!("file does not exist: {}", s)),
    }
}

fn parse_status_range(s: &str) -> Result<Vec<StatusRange>, String> {
    s.split(',')
        .map(|e| {
            let mut parts = e.splitn(2, '-').map(str::trim);
            let start: u16 = parts
                .next()
                .ok_or_else(|| format!("invalid status range: '{e}'"))?
                .parse()
                .map_err(|err| format!("invalid status range: '{e}' {err}"))?;
            match parts.next() {
                Some(end) => {
                    let end: u16 = end
                        .parse()
                        .map_err(|err| format!("invalid status range: '{e}' {err}"))?;
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
    ConfigLayer::command().debug_assert();
}

#[test]
fn verify_help() {
    let result = ConfigLayer::try_parse_from(["secret-scraper", "--help"]);
    // --help triggers a DisplayHelp error in clap, which is expected
    match result {
        Ok(_) => panic!("--help should exit with DisplayHelp"),
        Err(e) => match e.kind() {
            clap::error::ErrorKind::DisplayHelp => { /* expected */ }
            _ => panic!("unexpected error: {e}"),
        },
    }
}
