//! Error types used by the public SecretScraper facade API.

use std::{error::Error as StdError, fmt, io};

/// Error type returned by high-level SecretScraper library operations.
#[derive(Debug)]
pub enum SecretScraperError {
    /// Failure creating or running an async runtime.
    Runtime(String),
    /// Failure while scanning local files.
    Scanner(String),
    /// Failure while crawling web targets.
    Crawler(String),
    /// Failure while writing or formatting output.
    Output(String),
    /// I/O failure from files, streams, or paths.
    Io(io::Error),
    /// YAML serialization or deserialization failure.
    Yaml(serde_yaml::Error),
    /// CSV writer failure.
    Csv(csv::Error),
    /// Fallback error for external or unclassified failures.
    Other(String),
}

/// Convenient result alias for public SecretScraper APIs.
pub type Result<T> = std::result::Result<T, SecretScraperError>;

impl fmt::Display for SecretScraperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(message) => write!(f, "runtime error: {message}"),
            Self::Scanner(message) => write!(f, "scanner error: {message}"),
            Self::Crawler(message) => write!(f, "crawler error: {message}"),
            Self::Output(message) => write!(f, "output error: {message}"),
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Yaml(err) => write!(f, "YAML error: {err}"),
            Self::Csv(err) => write!(f, "CSV error: {err}"),
            Self::Other(message) => write!(f, "{message}"),
        }
    }
}

impl StdError for SecretScraperError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Yaml(err) => Some(err),
            Self::Csv(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for SecretScraperError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_yaml::Error> for SecretScraperError {
    fn from(value: serde_yaml::Error) -> Self {
        Self::Yaml(value)
    }
}

impl From<csv::Error> for SecretScraperError {
    fn from(value: csv::Error) -> Self {
        Self::Csv(value)
    }
}

impl From<anyhow::Error> for SecretScraperError {
    fn from(value: anyhow::Error) -> Self {
        Self::Other(format!("{:?}", value))
    }
}

impl From<String> for SecretScraperError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}

impl From<&str> for SecretScraperError {
    fn from(value: &str) -> Self {
        Self::Other(format!("{:?}", value))
    }
}
