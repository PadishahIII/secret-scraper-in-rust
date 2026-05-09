use std::collections::HashSet;

use anyhow::Result;
use globset::{Glob, GlobMatcher};
use url::Url;

pub trait URLFilter {
    fn filter(&self, url: &Url) -> bool;
}
pub enum DomainURLFilterType {
    BlackList,
    WhiteList,
}
use DomainURLFilterType::*;
pub struct DomainURLFilter {
    domain_list: Vec<GlobMatcher>,
    filter_type: DomainURLFilterType,
}

impl DomainURLFilter {
    pub fn new(
        domain_list: impl IntoIterator<Item = String>,
        filter_type: DomainURLFilterType,
    ) -> Result<Self> {
        let set: HashSet<String> = HashSet::from_iter(domain_list);
        let mut matchers = vec![];
        for pattern in set {
            matchers.push(Glob::new(&pattern)?.compile_matcher());
        }
        Ok(Self {
            domain_list: matchers,
            filter_type,
        })
    }
}
impl URLFilter for DomainURLFilter {
    fn filter(&self, url: &Url) -> bool {
        let matched = self
            .domain_list
            .iter()
            .any(|matcher| matcher.is_match(url.domain().unwrap_or("")));
        match self.filter_type {
            BlackList => !matched,
            WhiteList => matched,
        }
    }
}

pub struct ChainedURLFilter {
    filters: Vec<Box<dyn URLFilter>>,
}
pub struct ChainedURLFilterBuilder {
    filters: Vec<Box<dyn URLFilter>>,
}
impl ChainedURLFilterBuilder {
    pub fn add_blacklist(&mut self, domain_list: impl IntoIterator<Item = String>) -> Result<()> {
        self.filters.push(Box::new(DomainURLFilter::new(
            domain_list,
            DomainURLFilterType::BlackList,
        )?));
        Ok(())
    }
    pub fn add_whitelist(&mut self, domain_list: impl IntoIterator<Item = String>) -> Result<()> {
        self.filters.push(Box::new(DomainURLFilter::new(
            domain_list,
            DomainURLFilterType::WhiteList,
        )?));
        Ok(())
    }
    pub fn build(self) -> ChainedURLFilter {
        ChainedURLFilter {
            filters: self.filters,
        }
    }
}

impl ChainedURLFilter {
    pub fn builder() -> ChainedURLFilterBuilder {
        ChainedURLFilterBuilder { filters: vec![] }
    }
}
impl URLFilter for ChainedURLFilter {
    fn filter(&self, url: &Url) -> bool {
        if self.filters.is_empty() {
            true
        } else {
            self.filters.iter().all(|f| f.filter(url))
        }
    }
}
