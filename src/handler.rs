use std::error::Error;

use anyhow::Result;
use serde::Serialize;

use crate::cli::Rule;

#[derive(PartialEq, Eq, Hash, Serialize)]
pub struct Secret {
    type_: String,
    data: String,
}
pub trait Handler {
    fn handle(&self, text: &str) -> Result<Vec<Secret>>;
}

pub struct RegexHandler {
    rules: Vec<Rule>,
}
impl RegexHandler {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
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
                        type_: rule.name.clone(),
                        data: m.as_str().to_owned(),
                    })
                    .collect::<Vec<Secret>>()
            })
            .collect())
    }
}
