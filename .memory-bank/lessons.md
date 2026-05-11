## 2026-05-11 — Test Clap parsing at the real CLI seam

- Context: Added CLI integration tests under `tests/cli_tests.rs` for `CliConfigLayer::try_parse_from`.
- Memory: Clap parser behavior can differ from config-layer construction. In particular, `--status` previously panicked during parsing because the value parser returned `Vec<StatusRange>` while the field expected `StatusRangeRule`, and `value_delimiter=','` conflicted with the existing comma-aware parser.
- Evidence: Fixed `src/cli.rs` by parsing `--status` through a `StatusRangeRule` parser and removing Clap's duplicate comma delimiter; verified with `cargo fmt --check`, `cargo check`, and `cargo test`.
- Reuse: For future CLI option changes, add or update tests in `tests/cli_tests.rs` using `CliConfigLayer::try_parse_from` before relying on direct `CliConfigLayer` construction.

## 2026-05-11 — Boolean CLI flags must not clear YAML layers

- Context: User clarified all boolean CLI options must be flags, not explicit `true`/`false` value options.
- Memory: With Clap derive, `Option<bool>` plus `ArgAction::SetTrue` rejects explicit values and keeps a flag-only UX, but parsed absent flags can appear as `Some(false)`. Treat only `Some(true)` as present in `Config::apply_cli_layer`; otherwise YAML/default values must be preserved.
- Evidence: Added `tests/cli_tests.rs` coverage for explicit value rejection and YAML preservation, then fixed `Config::apply_cli_layer` via `apply_cli_bool`; verified with `cargo fmt --check`, `cargo check`, and `cargo test`.
- Reuse: When adding future boolean CLI flags, use `ArgAction::SetTrue`, assert `--flag true` is rejected, and merge only `Some(true)` into runtime config.

## 2026-05-11 — Release hardening requires sequential Cargo verification

- Context: Hardened the crate for production binary/library publication after a publish-readiness audit.
- Memory: Run release verification sequentially, not in parallel, because concurrent Cargo commands can make doctests fail with missing transient `rlib` paths. The reliable release gate is `cargo fmt --check`, `cargo check`, `cargo check --examples`, `cargo test`, `cargo test --doc`, `cargo package --list --allow-dirty`, and `cargo publish --dry-run --allow-dirty`.
- Evidence: Fixed `src/main.rs` panic-prone startup paths with controlled errors, added `tests/binary_tests.rs`, added `Cargo.toml` package metadata and root-relative `include`, and verified the final dry run packaged 34 files with no metadata warnings.
- Reuse: Before publishing, run the release gate sequentially and inspect `cargo package --list` for accidental local artifacts like `.memory-bank`, editor files, generated configs, or internal plans.
