//! URL filtering primitives used by the crawler.

use std::collections::HashSet;

use anyhow::Result;
use globset::{Glob, GlobMatcher};
use url::Url;

/// Predicate for deciding whether a URL should be accepted.
pub trait URLFilter {
    /// Return `true` when `url` should pass through the filter.
    fn filter(&self, url: &Url) -> bool;
}
/// Domain filter mode.
pub enum DomainURLFilterType {
    /// Reject URLs whose domain matches the configured patterns.
    BlackList,
    /// Accept only URLs whose domain matches the configured patterns.
    WhiteList,
}
use DomainURLFilterType::*;
/// Glob-based URL domain filter.
pub struct DomainURLFilter {
    domain_list: Vec<GlobMatcher>,
    filter_type: DomainURLFilterType,
}

impl DomainURLFilter {
    /// Create a domain filter from glob patterns and a filter mode.
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

/// Filter that evaluates several URL filters in sequence.
pub struct ChainedURLFilter {
    filters: Vec<Box<dyn URLFilter>>,
}
/// Builder for [`ChainedURLFilter`].
pub struct ChainedURLFilterBuilder {
    filters: Vec<Box<dyn URLFilter>>,
}
impl ChainedURLFilterBuilder {
    /// Add a blacklist domain filter to the chain.
    pub fn add_blacklist(&mut self, domain_list: impl IntoIterator<Item = String>) -> Result<()> {
        self.filters.push(Box::new(DomainURLFilter::new(
            domain_list,
            DomainURLFilterType::BlackList,
        )?));
        Ok(())
    }
    /// Add a whitelist domain filter to the chain.
    pub fn add_whitelist(&mut self, domain_list: impl IntoIterator<Item = String>) -> Result<()> {
        self.filters.push(Box::new(DomainURLFilter::new(
            domain_list,
            DomainURLFilterType::WhiteList,
        )?));
        Ok(())
    }
    /// Build the chained filter.
    pub fn build(self) -> ChainedURLFilter {
        ChainedURLFilter {
            filters: self.filters,
        }
    }
}

impl ChainedURLFilter {
    /// Create a new builder for chained URL filters.
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
