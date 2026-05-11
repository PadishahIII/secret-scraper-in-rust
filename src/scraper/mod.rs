//! Actor-based crawler internals.

/// Crawler actor message and result types.
pub mod bo;
/// Crawl scheduler and result aggregation.
pub mod crawler;
/// Actix worker actor that fetches and processes URLs.
pub mod worker;
