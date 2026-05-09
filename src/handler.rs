use anyhow::Result;
use derive_builder::Builder;
use serde::Serialize;

use crate::cli::Rule;

#[derive(PartialEq, Eq, Hash, Serialize, Builder)]
pub struct Secret {
    pub secret_type: String,
    pub data: String,
}
pub trait Handler: Send + Sync + 'static {
    fn handle(&self, text: &str) -> Result<Vec<Secret>>;
}

pub struct RegexHandler {
    rules: Vec<Rule>,
}
impl RegexHandler {
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
