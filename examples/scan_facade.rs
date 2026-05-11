use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use secret_scraper::{
    cli::Config,
    error::Result as SecretScraperResult,
    facade::{FileScannerFacade, ScanFacade, ScanResult},
};

fn main() -> Result<()> {
    let fixture = ScanFixture::create()?;
    let output = unique_temp_path("secret-scraper-scan-facade-output").with_extension("yml");

    let mut config = Config::default_with_rules();
    config.local = Some(fixture.root.clone());
    config.outfile = Some(output.clone());

    handle_scan_result(Box::new(FileScannerFacade::new(config)?).scan())?;

    let yaml = fs::read_to_string(&output)
        .with_context(|| format!("read scan output {}", output.display()))?;
    println!("scanned {}", fixture.root.display());
    println!("wrote {} bytes to {}", yaml.len(), output.display());
    println!("{}", preview(&yaml));

    fixture.cleanup();
    let _ = fs::remove_file(output);
    Ok(())
}
fn handle_scan_result(result: SecretScraperResult<ScanResult>) -> Result<()> {
    match result {
        Ok(res) => {
            match res {
                ScanResult::CrawlResult(res) => {
                    println!(
                        "got crawler result: {} domains, {} urls, {} secrets",
                        res.hosts.len(),
                        res.urls.len(),
                        res.secrets.values().map(|v| v.len()).sum::<usize>()
                    )
                }
                ScanResult::LocalScanResult(res) => {
                    println!(
                        "got local scan result: {} files, {} secrets",
                        res.len(),
                        res.values().map(|v| v.len()).sum::<usize>()
                    )
                }
            }
            Ok(())
        }
        Err(err) => {
            eprintln!("scan failed: {err}");
            Err(err.into())
        }
    }
}

fn preview(yaml: &str) -> String {
    yaml.lines().take(16).collect::<Vec<_>>().join("\n")
}

struct ScanFixture {
    root: PathBuf,
}

impl ScanFixture {
    fn create() -> Result<Self> {
        let root = unique_temp_path("secret-scraper-scan-facade");
        let nested = root.join("nested");
        fs::create_dir_all(&nested).with_context(|| format!("create {}", nested.display()))?;

        fs::write(
            root.join("config.txt"),
            r#"
            Contact: alice@example.com
            Swagger UI swaggerVersion
            rememberMe=deleteMe
            "#,
        )
        .context("write config fixture")?;

        fs::write(
            nested.join("application.js"),
            r#"
            const internal = "http://service.internal.local/api";
            const sourceMap = "/assets/application.js.map";
            const apiKey = "0123456789abcdef0123456789abcdef";
            "#,
        )
        .context("write nested fixture")?;

        Ok(Self { root })
    }

    fn cleanup(self) {
        let _ = fs::remove_dir_all(self.root);
    }
}

fn unique_temp_path(prefix: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    path.push(format!("{prefix}-{}-{nanos}", std::process::id()));
    path
}
