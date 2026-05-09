use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
    io,
    path::PathBuf,
};

use crate::{
    cli::StatusRangeRule,
    handler::Secret,
    urlparser::{ResponseStatus, URLNode},
};
use addr::parse_domain_name;
use anyhow::Result;
use csv::Writer;
use owo_colors::OwoColorize;

static UNKNOWN_HOST: &str = "UNKNOWN_HOST";
pub enum URLType {
    URL,
    JS,
}
impl AsRef<str> for URLType {
    fn as_ref(&self) -> &str {
        match self {
            URLType::URL => "URL",
            URLType::JS => "JS",
        }
    }
}
impl Display for URLType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            URLType::URL => write!(f, "URL"),
            URLType::JS => write!(f, "JS"),
        }
    }
}

pub struct Formatter {
    allowed_status: Option<StatusRangeRule>,
}
impl Formatter {
    pub fn new(allowed_status: Option<StatusRangeRule>) -> Self {
        Self { allowed_status }
    }
    pub fn format_status(&self, status: &ResponseStatus) -> String {
        format!("{status}").on_red().to_string()
    }
    pub fn format_normal_result(&self, content: &str) -> String {
        if content.is_empty() {
            return "".to_string();
        }
        content.bright_blue().to_string()
    }
    pub fn format_single_url(&self, url: &URLNode) -> String {
        format!(
            "{url} [{status}] [Content-Length: {cl}] [Content-Type: {ct}] [Title: {title}]",
            url = self.format_normal_result(&url.url),
            status = self.format_status(&url.response_status),
            cl = self.format_normal_result(
                &url.content_length
                    .map(|c| c.to_string())
                    .unwrap_or_default()
            ),
            ct = self.format_normal_result(&url.content_type.clone().unwrap_or_default()),
            title = self.format_normal_result(&url.title.clone().unwrap_or_default()),
        )
        .to_string()
    }
    pub fn format_found_domains(&self, found_urls: &[URLNode]) -> String {
        let urls_str = found_urls
            .iter()
            .map(|node| {
                let mut s = node.url_obj.host_str().unwrap_or(UNKNOWN_HOST).to_string();
                match node.url_obj.port_or_known_default() {
                    None => {}
                    Some(p) => {
                        s.push(':');
                        s.push_str(p.to_string().as_ref());
                    }
                }
                s
            })
            .collect::<HashSet<String>>()
            .into_iter()
            .collect::<Vec<String>>()
            .join("\n");
        format!(
            "{num} Domains:\n{urls}\n",
            num = found_urls.len(),
            urls = self.format_normal_result(&urls_str)
        )
        .to_string()
    }
    pub fn format_url_hierarchy(&self, urls: &HashMap<URLNode, Vec<URLNode>>) -> String {
        urls.iter()
            .map(|(base_url, child_urls)| {
                let children = child_urls
                    .iter()
                    .filter(|u| self.filter(u))
                    .map(|u| self.format_single_url(u))
                    .collect::<Vec<String>>();
                format!(
                    "{num} URLs from {base} [{base_status}] (depth:{base_depth}): \n{urls_str}",
                    num = children.len(),
                    base = base_url.url,
                    base_status = base_url.response_status,
                    base_depth = base_url.depth,
                    urls_str = children.join("\n")
                )
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
    pub fn format_url_per_domain(
        &self,
        domains: &HashSet<&str>,
        urls: &HashMap<URLNode, Vec<URLNode>>,
        url_type: URLType,
    ) -> String {
        let root_domains = domains
            .iter()
            .filter_map(|domain| get_root_domain(domain))
            .collect::<HashSet<String>>();
        let mut domain_urls: HashMap<String, Vec<&URLNode>> = HashMap::new();
        for (base_url, child_urls) in urls {
            let mut all_urls = Vec::with_capacity(child_urls.len() + 1);
            all_urls.push(base_url);
            all_urls.extend(child_urls.iter());

            for url in all_urls {
                if !self.filter(url) {
                    continue;
                }
                let domain = url
                    .url_obj
                    .host_str()
                    .and_then(get_root_domain)
                    .filter(|domain| root_domains.contains(domain))
                    .unwrap_or_else(|| "Other".to_string());
                domain_urls.entry(domain).or_default().push(url);
            }
        }

        let mut domains = domain_urls.keys().cloned().collect::<Vec<String>>();
        domains.sort();
        if let Some(other_pos) = domains.iter().position(|domain| domain == "Other") {
            let other = domains.remove(other_pos);
            domains.push(other);
        }

        domains
            .iter()
            .filter_map(|domain| {
                let urls = domain_urls.get(domain)?;
                if urls.is_empty() {
                    return None;
                }
                let urls_str = urls
                    .iter()
                    .map(|url| self.format_single_url(url))
                    .collect::<Vec<String>>()
                    .join("\n");
                Some(format!(
                    "{num} {url_type} from {domain}:\n{urls_str}",
                    num = urls.len(),
                    domain = domain
                ))
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
    pub fn format_js(&self, js_urls: &HashMap<URLNode, &Vec<URLNode>>) -> String {
        js_urls
            .iter()
            .map(|(base_url, child_urls)| {
                let child_urls = child_urls
                    .iter()
                    .filter(|u| self.filter(u))
                    .map(|u| format!("{url} [{res}]", url = u.url, res = u.response_status))
                    .collect::<Vec<String>>()
                    .join("\n");
                format!(
                    "{num} JS from {base}:\n{urls}",
                    num = child_urls.len(),
                    base = base_url.url,
                    urls = child_urls,
                )
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
    pub fn format_secrets(&self, url_secrets: &HashMap<URLNode, &Vec<Secret>>) -> String {
        let res = url_secrets
            .iter()
            .filter_map(|(url, secrets)| {
                if secrets.is_empty() {
                    return None;
                }
                Some(format!(
                    "{num} secrets found in {url} [{res}]:\n{secrets}",
                    num = secrets.len(),
                    url = url.url,
                    res = url.response_status,
                    secrets = secrets
                        .iter()
                        .map(|s| format!("{}: {}", s.secret_type, s.data))
                        .collect::<Vec<String>>()
                        .join("\n")
                ))
            })
            .collect::<Vec<String>>();
        if res.is_empty() {
            "No secrets found\n".to_string()
        } else {
            res.join("\n")
        }
    }
    pub fn format_local_secrets(&self, path_secrets: &HashMap<&PathBuf, &Vec<Secret>>) -> String {
        let res = path_secrets
            .iter()
            .filter_map(|(path, secrets)| {
                if secrets.is_empty() {
                    return None;
                }
                let mut res = format!(
                    "{num} secrets found in {path}:\n",
                    num = secrets.len(),
                    path = path.to_str()?,
                )
                .cyan()
                .to_string();
                res.push_str(
                    secrets
                        .iter()
                        .map(|s| format!("{}: {}", s.secret_type, s.data))
                        .collect::<Vec<String>>()
                        .join("\n")
                        .as_ref(),
                );
                Some(res)
            })
            .collect::<Vec<String>>();
        if res.is_empty() {
            "No secrets found\n".to_string()
        } else {
            res.join("\n")
        }
    }
    /// Report whether a url should be displayed
    pub fn filter(&self, url: &URLNode) -> bool {
        match url.response_status {
            ResponseStatus::Valid(c) => {
                if c == 404_u16 {
                    // filter out 404 by default
                    false
                } else {
                    match &self.allowed_status {
                        None => true, // no restriction
                        Some(allowed_status) => allowed_status.is_allowed(c),
                    }
                }
            }
            ResponseStatus::Unknown => true,
            ResponseStatus::Ignore => false,
            ResponseStatus::Failed(_) => false,
        }
    }
}
fn get_root_domain(host: &str) -> Option<String> {
    let domain = parse_domain_name(host).ok()?;
    domain.root().map(str::to_string)
}
pub fn output_csv(
    outfile: Box<dyn io::Write>,
    urls: &HashMap<URLNode, &Vec<URLNode>>,
    url_secrets: &HashMap<URLNode, &Vec<Secret>>,
) -> Result<u32> {
    let mut writer = Writer::from_writer(outfile);
    let mut count = 0;
    writer.write_record([
        "URL",
        "Title",
        "Response Code",
        "Content Length",
        "Content Type",
        "Secrets",
    ])?;
    for (url, secrets) in url_secrets {
        writer.write_record([
            url.url.to_owned(),
            url.title.clone().unwrap_or_default(),
            url.response_status.to_string(),
            url.content_length.unwrap_or_default().to_string(),
            url.content_type.clone().unwrap_or_default(),
            secrets
                .iter()
                .map(|s| format!("{}: {}", s.secret_type, s.data))
                .collect::<Vec<String>>()
                .join("\n"),
        ])?;
        count += 1;
    }
    for (url, children) in urls {
        if url_secrets.get(url).is_none() {
            writer.write_record([
                url.url.to_owned(),
                url.title.clone().unwrap_or_default(),
                url.response_status.to_string(),
                url.content_length.unwrap_or_default().to_string(),
                url.content_type.clone().unwrap_or_default(),
                "".to_string(),
            ])?;
            count += 1;
        }
        for url in *children {
            if url_secrets.get(url).is_none() {
                writer.write_record([
                    url.url.to_owned(),
                    url.title.clone().unwrap_or_default(),
                    url.response_status.to_string(),
                    url.content_length.unwrap_or_default().to_string(),
                    url.content_type.clone().unwrap_or_default(),
                    "".to_string(),
                ])?;
                count += 1;
            }
        }
    }
    writer.flush()?;
    Ok(count)
}
