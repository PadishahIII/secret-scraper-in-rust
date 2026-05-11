//! SecretScraper library API.
//!
//! The crate exposes configuration types, crawler/file-scan facades, URL
//! parsing utilities, output formatting, and lower-level crawler internals.

/// Command-line and YAML configuration types.
pub mod cli;
/// Library error and result types.
pub mod error;
/// High-level crawler and file-scanner facades.
pub mod facade;
/// URL filtering primitives.
pub mod filter;
/// Secret extraction handlers.
pub mod handler;
/// Tracing/logging initialization helpers.
pub mod logging;
/// Human-readable and CSV output formatting.
pub mod output;
/// Per-domain crawler rate limiting.
pub mod rate_limiter;
/// Local file scanning engine.
pub mod scanner;
/// Lower-level crawler actor implementation.
pub mod scraper;
/// URL node and link extraction utilities.
pub mod urlparser;
