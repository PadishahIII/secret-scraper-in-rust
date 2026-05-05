use std::error::Error;

use crate::cli::Rule;

#[derive(PartialEq, Eq, Hash)]
pub struct Secret<'a> {
    type_: &'a str,
    data: String,
}
pub trait Handler {
    fn handle(&self, text: &str) -> Result<impl IntoIterator<Item = Secret<'_>>, Box<dyn Error>>;
}

pub struct RegexHandler<'a> {
    rules: &'a Vec<Rule>,
}
impl<'a> RegexHandler<'a> {
    pub fn new(rules: &'a Vec<Rule>) -> Self {
        Self { rules }
    }
}
impl<'a> Handler for RegexHandler<'a> {
    fn handle(&self, text: &str) -> Result<impl IntoIterator<Item = Secret<'_>>, Box<dyn Error>> {
        Ok(self.rules.iter().flat_map(|rule| {
            rule.regex
                .find_iter(text)
                .map(|m| Secret {
                    type_: &rule.name,
                    data: m.as_str().to_owned(),
                })
                .collect::<Vec<Secret>>()
        }))
    }
}
