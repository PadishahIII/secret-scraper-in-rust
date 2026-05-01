use std::{path::PathBuf, str::FromStr};

use clap::{Parser, ValueEnum};
use pub_fields::pub_fields;

#[derive(Clone, Debug,Copy)]
pub(crate) enum StatusRange {
    Exact(u16),
    Range(u16, u16),
}
#[pub_fields]
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Config {
    #[arg(long, help = "Enable debug")]
    debug: bool,
    #[arg(short = 'a', long = "ua", help = "Set User-Agent")]
    user_agent: Option<String>,
    #[arg(short = 'c', long, help = "Set cookie")]
    cookie: Option<String>,
    #[arg(
        short = 'd',
        long,
        help = "Domain white list, wildcard(*) is supported, separated by commas, e.g. *.example.com, example*",
        value_parser = parse_domain_filter,
    )]
    allow_domains: Option<Vec<String>>,
    #[arg(
        short = 'D',
        long,
        help = "Domain black list, wildcard(*) is supported, separated by commas, e.g. *.example.com, example*",
        value_parser = parse_domain_filter,
    )]
    disallow_domains: Option<Vec<String>>,
    #[arg(short = 'f', long, 
        help = "Target urls file, separated by line break", 
        value_parser = existing_file)]
    url_file: Option<PathBuf>,
    #[arg(short = 'i', long, help = "Set config file, defaults to settings.yml", value_parser= existing_file)]
    config: Option<PathBuf>,
    #[arg(short='m', long, help="Set crawl mode, '1' for max_depth=1, '1' for max_depth=2, default '1'", value_enum, default_value_t=Mode::Normal, value_parser = clap::value_parser!(Mode))]
    mode: Mode,
    #[arg(long, help = "Max page number to crawl", default_value_t = 100000)]
    max_page: u32,
    #[arg(long, help = "Max total HTTP connections", default_value_t = 100)]
    max_connections: usize,
    #[arg(long, help = "Max keep-alive HTTP connections", default_value_t = 50)]
    max_keepalive_connections: usize,
    #[arg(long, help = "Max keep-alive HTTP connections per domain")]
    max_concurrency_per_domain: Option<usize>,
    #[arg(
        long,
        help = "Minimum seconds between requests to the same domain",
    )]
    min_request_interval: Option<f32>,
    #[arg(
        long,
        short = 'o',
        help = "Output result to specified file in csv format"
    )]
    outfile: Option<PathBuf>,
    #[arg(
        short,
        long="status",
        help = "Filter response status to display, seperated by commas, e.g. 200,300-400",
        value_parser = parse_status_range,
    )]
    status_filter: Option<Vec<StatusRange>>,
    #[arg(
        short = 'x',
        long,
        help = "Set proxy, e.g. http://127.0.0.1:8080, socks5://127.0.0.1:7890"
    )]
    proxy: Option<String>,
    #[arg(short = 'H', long, help = "Hide regex search result")]
    hide_regex: bool,
    #[arg(short = 'F', long, help = "Follow redirects")]
    follow_redirect: bool,
    #[arg(short, long, help = "Target URL")]
    url: String,
    #[arg(long, help = "Show detailed result")]
    detail: bool,
    #[arg(long, help = "Validate the status of found urls")]
    validate: bool,
    #[arg(
        short,
        long,
        help = "Local file or directory, scan local file/directory recursively",
        value_parser = existing_file,
    )]
    local: Option<PathBuf>,
}
#[derive(Debug, ValueEnum, Clone)]
pub(crate) enum Mode {
    #[value(name = "1")]
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
            _ => Err(format!("invalid mode: {}", s))
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
fn parse_status_range(s:&str) -> Result<Vec<StatusRange>, String>{
    s.split(',')
        .map(|e|{
            let mut parts = e.splitn(2, '-').map(str::trim);
            let start:u16 = parts.next()
                .ok_or_else(||format!("invalid status range: '{e}'"))?
                .parse()
                .map_err(|err|format!("invalid status range: '{e}' {err}"))?;
            match parts.next(){
                Some(end) => {
                    let end:u16 = end.parse().map_err(|err|format!("invalid status range: '{e}' {err}"))?;
                    Ok(StatusRange::Range(start, end))
                }
                None => {
                    Ok(StatusRange::Exact(start))
                }
            }
        })
            .collect()
        
}

#[test]
fn verify_cli() {
    Config::command().debug_assert();
}
#[test]
fn verify_help() {
    Config::try_parse_from(["secret-scraper", "--help"]).unwrap();
}
