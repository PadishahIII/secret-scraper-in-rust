use std::{error::Error as StdError, fmt, io};

#[derive(Debug)]
pub enum SecretScraperError {
    Runtime(String),
    Scanner(String),
    Crawler(String),
    Output(String),
    Io(io::Error),
    Yaml(serde_yaml::Error),
    Csv(csv::Error),
    Other(String),
}

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
        Self::Other(value.to_string())
    }
}

impl From<String> for SecretScraperError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}

impl From<&str> for SecretScraperError {
    fn from(value: &str) -> Self {
        Self::Other(value.to_string())
    }
}
