use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

fn binary_path() -> &'static str {
    env!("CARGO_BIN_EXE_secret_scraper")
}

fn unique_temp_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("secret_scraper_binary_tests_{name}_{nanos}"))
}

fn temp_file(name: &str, content: &str) -> PathBuf {
    let path = unique_temp_path(name);
    fs::write(&path, content).expect("write temp file");
    path
}

fn temp_dir(name: &str) -> PathBuf {
    let path = unique_temp_path(name);
    fs::create_dir(&path).expect("create temp dir");
    path
}

fn run(args: &[&str]) -> Output {
    Command::new(binary_path())
        .args(args)
        .output()
        .expect("run secret_scraper binary")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn remove_file_if_exists(path: &Path) {
    let _ = fs::remove_file(path);
}

fn remove_dir_if_exists(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

#[test]
fn invalid_yaml_layer_exits_with_error_without_panic() {
    let config = temp_file(
        "invalid_yaml",
        r#"
urlFind: []
urlFind:
  - "https?://[^\\s\"']+"
jsFind: []
headers:
  "bad header": "value"
url: "https://example.com"
"#,
    );
    let output = run(&[
        "--config",
        config.to_str().unwrap(),
        "--url",
        "https://example.com",
    ]);
    let stderr = stderr(&output);

    assert!(!output.status.success());
    assert!(stderr.contains("configuration error:"), "{stderr}");
    assert!(!stderr.contains("panicked at"), "{stderr}");

    remove_file_if_exists(&config);
}

#[test]
fn invalid_output_path_exits_with_error_without_panic() {
    let config = temp_file(
        "local_config",
        r#"
urlFind: []
jsFind: []
"#,
    );
    let local = temp_file("local_scan_input", "SECRET_TOKEN_1234567890\n");
    let missing_dir = unique_temp_path("missing_dir");
    let outfile = missing_dir.join("out.yml");
    let output = run(&[
        "--config",
        config.to_str().unwrap(),
        "--local",
        local.to_str().unwrap(),
        "--outfile",
        outfile.to_str().unwrap(),
    ]);
    let stderr = stderr(&output);

    assert!(!output.status.success());
    assert!(stderr.contains("scan setup error:"), "{stderr}");
    assert!(!stderr.contains("panicked at"), "{stderr}");
    assert!(
        !stdout(&output).contains("Start to scan local files..."),
        "{:?}",
        stdout(&output)
    );

    remove_file_if_exists(&config);
    remove_file_if_exists(&local);
}

#[test]
fn prints_gray_startup_banner_after_resolving_url_config() {
    let config = temp_file(
        "banner_url_config",
        r#"
urlFind:
  - "https?://[^\\s\"']+"
jsFind: []
max_page: 1000
outfile: "1.csv"
rules:
  - name: "Token"
    regex: "TOKEN_[A-Z0-9]+"
    loaded: true
"#,
    );
    let output = run(&[
        "--config",
        config.to_str().unwrap(),
        "--url",
        "http://127.0.0.1:9",
        "--max-depth",
        "1",
        "--max-page",
        "0",
    ]);
    let stdout = stdout(&output);

    assert!(
        stdout.contains("\u{1b}[90mTarget urls num: 1"),
        "{stdout:?}"
    );
    assert!(
        stdout.contains("Max depth: 1, Max page num: 0"),
        "{stdout:?}"
    );
    assert!(
        stdout.contains("Output file: 1.csv\u{1b}[39m"),
        "{stdout:?}"
    );
    assert!(stdout.contains("Start to crawl..."), "{stdout:?}");
    assert!(!stdout.contains("panicked at"), "{stdout:?}");

    remove_file_if_exists(&config);
    remove_file_if_exists(Path::new("1.csv"));
}

#[test]
fn startup_banner_counts_url_file_targets_and_mode_depth() {
    let urls = temp_file(
        "banner_urls",
        "https://example.com\n\nhttps://example.org\n",
    );
    let config = temp_file(
        "banner_url_file_config",
        r#"
urlFind: []
jsFind: []
max_page: 7
"#,
    );
    let output = run(&[
        "--config",
        config.to_str().unwrap(),
        "--url-file",
        urls.to_str().unwrap(),
        "--mode",
        "thorough",
    ]);
    let stdout = stdout(&output);

    assert!(
        stdout.contains("\u{1b}[90mTarget urls num: 2"),
        "{stdout:?}"
    );
    assert!(
        stdout.contains("Max depth: 2, Max page num: 7"),
        "{stdout:?}"
    );
    assert!(
        stdout.contains("Output file: stdout\u{1b}[39m"),
        "{stdout:?}"
    );

    remove_file_if_exists(&config);
    remove_file_if_exists(&urls);
}

#[test]
fn startup_banner_counts_single_local_file_target() {
    let config = temp_file(
        "banner_local_file_config",
        r#"
urlFind: []
jsFind: []
rules:
  - name: "Token"
    regex: "SECRET_TOKEN_[0-9]+"
    loaded: true
"#,
    );
    let local = temp_file("banner_local_file", "SECRET_TOKEN_1234567890\n");
    let output_path = unique_temp_path("banner_local_file_output.yml");
    let output = run(&[
        "--config",
        config.to_str().unwrap(),
        "--local",
        local.to_str().unwrap(),
        "--outfile",
        output_path.to_str().unwrap(),
    ]);
    let stdout = stdout(&output);

    assert!(
        stdout.contains("\u{1b}[90mTarget files num: 1"),
        "{stdout:?}"
    );
    assert!(
        stdout.contains("Max depth: N/A, Max page num: 1000"),
        "{stdout:?}"
    );
    assert!(
        stdout.contains(&format!("Output file: {}\u{1b}[39m", output_path.display())),
        "{stdout:?}"
    );
    assert!(
        stdout.contains("Start to scan local files..."),
        "{stdout:?}"
    );

    remove_file_if_exists(&config);
    remove_file_if_exists(&local);
    remove_file_if_exists(&output_path);
}

#[test]
fn startup_banner_counts_recursive_local_directory_files() {
    let config = temp_file(
        "banner_local_dir_config",
        r#"
urlFind: []
jsFind: []
"#,
    );
    let root = temp_dir("banner_local_dir");
    fs::write(root.join("first.txt"), "FIRST_SECRET_1234567890").expect("write first file");
    fs::create_dir(root.join("nested")).expect("create nested dir");
    fs::write(
        root.join("nested").join("second.txt"),
        "SECOND_SECRET_1234567890",
    )
    .expect("write second file");
    let output_path = unique_temp_path("banner_local_dir_output.yml");
    let output = run(&[
        "--config",
        config.to_str().unwrap(),
        "--local",
        root.to_str().unwrap(),
        "--outfile",
        output_path.to_str().unwrap(),
    ]);
    let stdout = stdout(&output);

    assert!(
        stdout.contains("\u{1b}[90mTarget files num: 2"),
        "{stdout:?}"
    );
    assert!(
        stdout.contains("Max depth: N/A, Max page num: 1000"),
        "{stdout:?}"
    );

    remove_file_if_exists(&config);
    remove_file_if_exists(&output_path);
    remove_dir_if_exists(&root);
}
