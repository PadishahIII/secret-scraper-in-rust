use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use secret_scraper::{
    cli::{Config, Mode},
    error::Result as SecretScraperResult,
    facade::{CrawlerFacade, ScanFacade, ScanResult},
};

fn main() -> Result<()> {
    let server = ExampleServer::start()?;

    run_crawler_example(Mode::Normal, server.url("/"))?;
    run_crawler_example(Mode::Thorough, server.url("/"))?;

    Ok(())
}

fn run_crawler_example(mode: Mode, seed_url: String) -> Result<()> {
    let outfile = example_output_path(match mode {
        Mode::Normal => "normal",
        Mode::Thorough => "thorough",
    });
    let _ = fs::remove_file(&outfile);

    let mut config = Config::default_with_rules();
    config.url = Some(seed_url);
    config.mode = mode;
    config.outfile = Some(outfile.clone());
    config.detail = true;
    config.max_page = Some(20);
    config.max_concurrency_per_domain = 4;
    config.min_request_interval = Duration::from_millis(0);
    config.validate = true;
    handle_scan_result(Box::new(CrawlerFacade::new(config)?).scan())?;

    let csv = fs::read_to_string(&outfile)
        .with_context(|| format!("read crawler output {}", outfile.display()))?;
    println!("wrote {} bytes to {}", csv.len(), outfile.display());
    println!("{}", preview(&csv));
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

fn preview(csv: &str) -> String {
    csv.lines().take(6).collect::<Vec<_>>().join("\n")
}

fn example_output_path(label: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    path.push(format!("secret-scraper-crawler-{label}-{nanos}.csv"));
    path
}

struct ExampleServer {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl ExampleServer {
    fn start() -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").context("bind example server")?;
        listener
            .set_nonblocking(true)
            .context("set example server nonblocking")?;
        let addr = listener.local_addr().context("read example server addr")?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let server_shutdown = shutdown.clone();
        let routes = Arc::new(routes());

        let handle = thread::spawn(move || {
            while !server_shutdown.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let routes = routes.clone();
                        thread::spawn(move || handle_connection(stream, &routes));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            addr,
            shutdown,
            handle: Some(handle),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{}", self.addr, path)
    }
}

impl Drop for ExampleServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(self.addr);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn routes() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        (
            "/",
            r#"
            <html>
              <head><title>Example Home</title></head>
              <body>
                <a href="/level-1">level one</a>
                <a href="/openapi">swagger</a>
                Contact: demo@example.com
              </body>
            </html>
            "#,
        ),
        (
            "/level-1",
            r#"
            <html>
              <head><title>Level One</title></head>
              <body>
                <a href="/level-2">level two</a>
                <script src="/assets/app.js"></script>
                rememberMe=deleteMe
              </body>
            </html>
            "#,
        ),
        (
            "/level-2",
            r#"
            <html>
              <head><title>Level Two</title></head>
              <body>
                <a href="/level-3">level three</a>
                Internal service: 192.168.1.42
                "0123456789abcdef0123456789abcdef"
              </body>
            </html>
            "#,
        ),
        (
            "/level-3",
            r#"
            <html>
              <head><title>Level Three</title></head>
              <body>
                Deep page visible in thorough mode.
                Source map: /assets/app.js.map
              </body>
            </html>
            "#,
        ),
        (
            "/openapi",
            r#"
            <html>
              <head><title>Swagger Fixture</title></head>
              <body>
                Swagger UI swaggerVersion
              </body>
            </html>
            "#,
        ),
        (
            "/assets/app.js",
            r#"
            const api = "http://api.example.test/v1/users";
            //# sourceMappingURL=/assets/app.js.map
            "#,
        ),
    ])
}

fn handle_connection(mut stream: TcpStream, routes: &HashMap<&'static str, &'static str>) {
    let mut buffer = [0_u8; 4096];
    let bytes = stream.read(&mut buffer).unwrap_or(0);
    let request = String::from_utf8_lossy(&buffer[..bytes]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    let (status, body) = routes
        .get(path)
        .map(|body| ("200 OK", *body))
        .unwrap_or(("404 Not Found", "not found"));

    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
}
