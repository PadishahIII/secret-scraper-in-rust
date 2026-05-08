use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use anyhow::bail;
use derive_builder::Builder;
use tokio::{sync, time::sleep};
use url::Url;

struct DomainState {
    sema: sync::Semaphore,
    last_request_started_at: sync::Mutex<Instant>,
}
// TODO: use arc to reuse among workers
#[derive(Builder)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct DomainRateLimiter<'a> {
    #[builder(default = 5)]
    max_concurrency_per_domain: usize,
    #[builder(default = Duration::from_millis(200) )]
    min_interval: Duration,

    #[builder(setter(skip), default)]
    states: HashMap<&'a str, DomainState>,
}
impl<'a> DomainRateLimiterBuilder<'a> {
    fn validate(&self) -> Result<(), String> {
        if let Some(m) = self.max_concurrency_per_domain
            && m == 0
        {
            return Err(format!("max_concurrency_per_domain must be at least 1"));
        }
        Ok(())
    }
}
impl<'a> DomainRateLimiter<'a> {
    pub async fn acquire(&mut self, domain: &'a str) -> anyhow::Result<()> {
        let state = self.states.entry(domain).or_insert_with(|| DomainState {
            sema: sync::Semaphore::new(self.max_concurrency_per_domain),
            last_request_started_at: sync::Mutex::new(Instant::now()),
        });
        let _permit = state.sema.acquire().await?;
        let mut m = state.last_request_started_at.lock().await;
        if let Some(wait) = self.min_interval.checked_sub(m.elapsed()) {
            sleep(wait).await;
        }
        *m = Instant::now();
        Ok(())
    }
}
