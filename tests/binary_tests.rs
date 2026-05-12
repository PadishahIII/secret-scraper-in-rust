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
