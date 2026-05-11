# Graceful Shutdown Design

## Goal

When a user presses Ctrl-C during a crawler or local file scan, SecretScraper should stop scheduling new work, allow already-started work to finish briefly, and return partial results through the existing facade and output paths. Graceful shutdown should preserve useful data without hanging indefinitely.

This design covers graceful shutdown for both web crawling and local file scanning.

## Current Context

`main.rs` builds a `Config`, initializes tracing, constructs either `FileScannerFacade` or `CrawlerFacade`, then calls `facade.scan()`. `CrawlerFacade` owns an Actix `SystemRunner` and a `Crawler`, and `CrawlerFacade::scan()` runs `crawler.run().await`, converts the crawler state into `CrawlerResult`, and then logs or writes output.

`FileScannerFacade` builds a `FileScanner` from either a single local file or a globbed list of files, creates a Tokio runtime, calls `scanner.scan().await`, logs formatted secrets, and writes the local scan result as YAML. `FileScanner::scan()` currently walks targets sequentially: it reads one file, runs the regex handler in `spawn_blocking`, records the file's result, then moves to the next file.

`Crawler::run()` currently loops until its queue is empty, its page limit is reached, or no work remains. It schedules fetch work through Actix worker actors and consumes worker results through an `mpsc` channel. `Crawler::consume()` is the single path that mutates response status, discovered links, and secrets. Validation uses a second scheduling loop over URLs whose status is still `Unknown`.

## Public API Shape

Add a small cooperative cancellation primitive:

```rust
#[derive(Clone, Default)]
pub struct ShutdownToken {
    cancelled: Arc<AtomicBool>,
}

impl ShutdownToken {
    pub fn new() -> Self;
    pub fn cancel(&self);
    pub fn is_cancelled(&self) -> bool;
}
```

The default token starts uncancelled. Clones observe the same cancellation flag.

Keep existing `new(config)` constructors for compatibility and make them use a default token. Add `CrawlerFacade::with_shutdown(config, token)` and `FileScannerFacade::with_shutdown(config, token)` for the CLI and tests. Add matching builder or constructor support so the token is stored on both `Crawler` and `FileScanner`.

## CLI Flow

`main()` should create one `ShutdownToken` before constructing either facade. Install a Ctrl-C handler that calls `token.cancel()`. Use the small `ctrlc` crate for signal handling unless implementation discovery shows an existing dependency already provides equivalent synchronous Ctrl-C handling. Then construct `CrawlerFacade::with_shutdown(config, token)` for crawler mode or `FileScannerFacade::with_shutdown(config, token)` for local scan mode.

The handler should not perform I/O, allocate complex state, or stop the process. It only marks the token. The active scan loop handles the actual shutdown.

## Crawler Run Behavior

`Crawler::run()` should remain the main orchestration point.

Before scheduling each URL, the producer loop checks `shutdown_token.is_cancelled()`. If cancellation has been requested, it stops pulling from `working_queue` and moves into draining mode.

In draining mode, the crawler continues receiving completed in-flight worker results and passes them through `consume()`. This preserves the current invariant that result state is updated in one place. The crawler should wait only up to a fixed drain timeout, initially two seconds. If in-flight tasks finish before the timeout, the run returns immediately. If the timeout expires, the run returns partial state with whatever has already been consumed.

If shutdown is requested before validation starts, validation should be skipped. If shutdown is requested during validation, the validation loop should stop scheduling new validation fetches and briefly drain already-started validation requests using the same timeout policy.

Graceful shutdown is not an error. `Crawler::run()` should return `Ok(())` after graceful shutdown so `CrawlerFacade::scan()` can convert and emit partial results.

## File Scanner Behavior

`FileScanner::scan()` should check `shutdown_token.is_cancelled()` before starting each target. If cancellation has been requested, it stops scanning additional files and returns the results accumulated so far.

If cancellation is requested while a file read or `spawn_blocking` handler call is already in progress, the scanner should let that file finish and record its result before checking the token again. This mirrors the crawler's drain policy at file granularity without adding hard cancellation for in-progress filesystem or CPU work.

The scanner should preserve the existing output shape: scanned files map to their discovered secrets. Files that were never started because shutdown was requested should not be included in the partial result. This differs from today's initialization, which pre-populates every target with an empty secret set; the implementation should switch to inserting a target only after that target has been processed.

Graceful shutdown is not an error for local scans. `FileScannerFacade::scan()` should still log and write the partial YAML result, then return `Ok(ScanResult::LocalScanResult(...))`.

## Result Semantics

Partial crawler results use existing data structures and status values:

- Completed successful requests remain `ResponseStatus::Valid(...)`.
- Completed failed requests remain `ResponseStatus::Failed(...)`.
- Ignored content remains `ResponseStatus::Ignore`.
- URLs discovered but not fetched before shutdown may remain `ResponseStatus::Unknown`.
- URLs never scheduled because shutdown was requested are not artificially added.

The crawler facade should log that shutdown was requested and partial crawl results are being returned. The file scanner facade should log that shutdown was requested and partial local scan results are being returned. Output formats do not change.

## Timeout Policy

Use a hard-coded two-second drain timeout for crawler in-flight request draining in the first implementation. This avoids new CLI and YAML configuration surface while preventing indefinite shutdown waits. Existing request timeouts continue to bound individual HTTP requests; the drain timeout bounds how long graceful shutdown waits for worker results after cancellation.

Local file scanning does not need a separate drain timeout in the first implementation because it is sequential. Once a file has started, its current read and regex handler execution finish normally, then the scanner observes the cancellation before starting the next file.

Configurable shutdown policies can be added later if users need them.

## Error Handling

Worker fetch and process errors continue to be represented as URL response statuses through existing `consume()` and validation result handling. File scanner read and handler errors remain scanner errors. Shutdown itself should not become a `SecretScraperError`.

If installing the Ctrl-C handler fails, the CLI should report a startup error before scanning begins. Library callers using `CrawlerFacade::new(config)` or `FileScannerFacade::new(config)` are unaffected and receive an uncancelled token.

## Testing

Add tests that cover:

1. Cancellation before a crawl starts returns without hanging and does not schedule fetches.
2. Cancellation during in-flight fetches drains completed in-flight work and stops scheduling newly discovered children.
3. Shutdown requested before validation prevents validation fetches from starting.
4. Shutdown requested during validation stops scheduling additional validation work and drains already-started validation fetches briefly.
5. File scanner cancellation before a target starts returns only already-processed files.
6. File scanner cancellation during a target records that target if it completes successfully, then stops before later targets.
7. Normal crawler, file scanner, and facade behavior is unchanged when the token is never cancelled.
8. `CrawlerFacade::with_shutdown` returns `Ok(ScanResult::CrawlResult(...))` after shutdown so callers can inspect partial results.
9. `FileScannerFacade::with_shutdown` returns `Ok(ScanResult::LocalScanResult(...))` after shutdown so callers can inspect partial results.

Crawler tests should use local test servers and short timeouts. File scanner tests should use temporary local files and deterministic handlers. Tests should not depend on real Ctrl-C delivery; unit and integration tests can cancel a cloned `ShutdownToken` directly for deterministic behavior.

## Non-Goals

- No configurable shutdown mode or timeout in CLI/YAML yet.
- No output format changes.
- No hard cancellation of already-running reqwest requests or Actix actors unless a future need appears.
- No hard cancellation of already-running file reads or blocking regex handler tasks.

## Open Decisions Resolved

The chosen behavior is cooperative drain shutdown: stop scheduling new work, let already-started work complete within the mode's policy, and return partial results.
