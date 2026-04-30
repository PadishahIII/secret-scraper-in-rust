use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use pub_fields::pub_fields;

const PROGRAM_NAME: &str = "secret-scraper";

#[pub_fields]
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    #[arg(long, help = "Enable debug")]
    debug: bool,
    #[arg(short = 'a', long = "ua", help = "Set User-Agent")]
    user_agent: Option<String>,
    #[arg(short = 'c', long, help = "Set cookie")]
    cookie: Option<String>,
    #[arg(
        short = 'd',
        long,
        help = "Domain white list, wildcard(*) is supported, separated by commas, e.g. *.example.com, example*"
    )]
    allow_domains: Option<String>,
    #[arg(
        short = 'D',
        long,
        help = "Domain black list, wildcard(*) is supported, separated by commas, e.g. *.example.com, example*"
    )]
    disallow_domains: Option<String>,
    #[arg(short = 'f', long, help = "Target urls file, separated by line break")]
    url_file: Option<PathBuf>,
    #[arg(short = 'i', long, help = "Set config file, defaults to settings.yml")]
    config: Option<PathBuf>,
    #[arg(short='m', long, help="Set crawl mode, '1' for max_depth=1, '1' for max_depth=2, default '1'", value_enum, default_value_t=Mode::Normal)]
    mode: Mode,
    #[arg(long, help = "Max page number to crawl", default_value_t = 100000)]
    max_page: u32,
    #[arg(long, help = "Max total HTTP connections", default_value_t = 100)]
    max_connections: usize,
    #[arg(long, help = "Max keep-alive HTTP connections", default_value_t = 50)]
    max_keepalive_connections: usize,
    #[arg(long, help = "Max keep-alive HTTP connections", default_value_t = 2)]
    max_concurrency_per_domain: usize,
    #[arg(
        long,
        help = "Minimum seconds between requests to the same domain",
        default_value_t = 0.2
    )]
    min_request_interval: f32,
    #[arg(
        long,
        short = 'o',
        help = "Output result to specified file in csv format"
    )]
    outfile: Option<PathBuf>,
    #[arg(
        short,
        long,
        help = "Filter response status to display, seperated by commas, e.g. 200,300-400"
    )]
    status: Option<String>,
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
        help = "Local file or directory, scan local file/directory recursively"
    )]
    local: Option<PathBuf>,
}
#[derive(Debug, ValueEnum, Clone)]
pub enum Mode {
    #[value(name = "1")]
    Normal,
    #[value(name = "2")]
    Thorough,
}

#[test]
fn verify_cli() {
    Cli::command().debug_assert();
}
#[test]
fn verify_help() {
    Cli::try_parse_from(["secret-scraper", "--help"]).unwrap();
}
