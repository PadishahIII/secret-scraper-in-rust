use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use secret_scraper::{
    cli::{Config, Rule},
    facade::{FileScannerFacade, ScanFacade, ScanResult},
};
use tokio_util::sync::CancellationToken;

fn test_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    path.push(format!("secret-scraper-file-facade-{name}-{nanos}"));
    path
}

fn scanner_config(local: PathBuf, outfile: PathBuf) -> Config {
    Config {
        local: Some(local),
        outfile: Some(outfile),
        custom_rules: vec![
            Rule::new("api_key".to_string(), r"SECRET_[A-Z0-9]+").expect("api key rule"),
            Rule::new("token".to_string(), r"TOKEN_[A-Z0-9]+").expect("token rule"),
        ],
        ..Config::default()
    }
}

fn run_file_facade(config: Config) {
    let _ = run_file_facade_with_shutdown(config, CancellationToken::new());
}

fn run_file_facade_with_shutdown(config: Config, shutdown: CancellationToken) -> ScanResult {
    Box::new(FileScannerFacade::with_shutdown(config, shutdown).expect("file scanner facade"))
        .scan()
        .expect("scan file facade")
}

fn read_output(path: &Path) -> String {
    fs::read_to_string(path).expect("read facade output")
}

#[test]
fn file_scanner_facade_scans_single_file_and_writes_yaml() {
    let input = test_path("single-input.txt");
    let output = test_path("single-output.yml");
    fs::write(&input, "prefix SECRET_ALPHA middle TOKEN_BETA suffix").expect("write input");

    run_file_facade(scanner_config(input.clone(), output.clone()));

    let yaml = read_output(&output);
    assert!(
        yaml.contains(input.to_str().expect("utf8 input path")),
        "{yaml}"
    );
    assert!(yaml.contains("secret_type: api_key"), "{yaml}");
    assert!(yaml.contains("data: SECRET_ALPHA"), "{yaml}");
    assert!(yaml.contains("secret_type: token"), "{yaml}");
    assert!(yaml.contains("data: TOKEN_BETA"), "{yaml}");

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(output);
}

#[test]
fn file_scanner_facade_recursively_scans_directory_files() {
    let root = test_path("dir");
    let nested = root.join("nested");
    fs::create_dir_all(&nested).expect("create nested dir");
    let first = root.join("first.txt");
    let second = nested.join("second.txt");
    let output = test_path("dir-output.yml");
    fs::write(&first, "SECRET_FIRST").expect("write first file");
    fs::write(&second, "TOKEN_SECOND").expect("write second file");

    run_file_facade(scanner_config(root.clone(), output.clone()));

    let yaml = read_output(&output);
    assert!(
        yaml.contains(first.to_str().expect("utf8 first path")),
        "{yaml}"
    );
    assert!(
        yaml.contains(second.to_str().expect("utf8 second path")),
        "{yaml}"
    );
    assert!(yaml.contains("data: SECRET_FIRST"), "{yaml}");
    assert!(yaml.contains("data: TOKEN_SECOND"), "{yaml}");

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_file(output);
}

#[test]
fn file_scanner_facade_requires_local_path() {
    let output = test_path("missing-local-output.yml");
    let config = Config {
        outfile: Some(output.clone()),
        custom_rules: vec![Rule::new("secret".to_string(), r"SECRET_[A-Z0-9]+").expect("rule")],
        ..Config::default()
    };

    let err = match FileScannerFacade::new(config) {
        Ok(_) => panic!("missing local should fail"),
        Err(err) => err,
    };

    assert!(err.to_string().contains("'local' (base dir) not set"));
    assert!(!output.exists());
}

#[test]
fn file_scanner_facade_truncates_existing_output_file() {
    let input = test_path("truncate-input.txt");
    let output = test_path("truncate-output.yml");
    fs::write(&input, "SECRET_NEW").expect("write input");
    fs::write(&output, "stale SECRET_OLD content that should be removed")
        .expect("write stale output");

    run_file_facade(scanner_config(input.clone(), output.clone()));

    let yaml = read_output(&output);
    assert!(yaml.contains("data: SECRET_NEW"), "{yaml}");
    assert!(!yaml.contains("SECRET_OLD"), "{yaml}");

    let _ = fs::remove_file(input);
    let _ = fs::remove_file(output);
}

#[test]
fn file_scanner_facade_shutdown_before_start_writes_empty_partial_result() {
    let root = test_path("shutdown-dir");
    fs::create_dir_all(&root).expect("create shutdown dir");
    let first = root.join("first.txt");
    let output = test_path("shutdown-output.yml");
    fs::write(&first, "SECRET_FIRST").expect("write first file");
    let shutdown = CancellationToken::new();
    shutdown.cancel();

    let result =
        run_file_facade_with_shutdown(scanner_config(root.clone(), output.clone()), shutdown);

    let ScanResult::LocalScanResult(result) = result else {
        panic!("expected local scan result");
    };
    assert!(result.is_empty(), "expected no files to be scanned");
    let yaml = read_output(&output);
    assert!(!yaml.contains("SECRET_FIRST"), "{yaml}");

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_file(output);
}
