//! Secret detection handlers and result types.

use anyhow::Result;
use derive_builder::Builder;
use serde::Serialize;

use crate::cli::Rule;

/// A secret detected by a handler.
#[derive(PartialEq, Eq, Hash, Serialize, Builder)]
#[allow(missing_docs)]
pub struct Secret {
    /// Human-readable rule or secret category name.
    pub secret_type: String,
    /// Matched secret data.
    pub data: String,
}
/// Trait implemented by text scanners that emit detected secrets.
pub trait Handler: Send + Sync + 'static {
    /// Scan `text` and return all detected secrets.
    fn handle(&self, text: &str) -> Result<Vec<Secret>>;
}

/// Regex-backed [`Handler`] using configured [`Rule`] values.
pub struct RegexHandler {
    rules: Vec<Rule>,
}
impl RegexHandler {
    /// Build a regex handler from non-empty rules.
    pub fn new(rules: Vec<Rule>) -> Result<Self> {
        (!rules.is_empty())
            .then_some(())
            .ok_or(anyhow::anyhow!("no rule specified"))?;
        Ok(Self { rules })
    }
}
impl Handler for RegexHandler {
    fn handle(&self, text: &str) -> Result<Vec<Secret>> {
        Ok(self
            .rules
            .iter()
            .flat_map(|rule| {
                rule.regex
                    .find_iter(text)
                    .map(|m| Secret {
                        secret_type: rule.name.clone(),
                        data: m.as_str().to_owned(),
                    })
                    .collect::<Vec<Secret>>()
            })
            .collect())
    }
}
