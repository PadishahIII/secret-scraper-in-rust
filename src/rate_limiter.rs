use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use derive_builder::Builder;
use tokio::{
    sync::{self, SemaphorePermit},
    time::sleep,
};

struct DomainState {
    sema: sync::Semaphore,
    last_request_started_at: sync::Mutex<Instant>,
}
#[derive(Builder)]
#[builder(build_fn(validate = "Self::validate"))]
pub struct DomainRateLimiter {
    #[builder(default = 5)]
    max_concurrency_per_domain: usize,
    #[builder(default = Duration::from_millis(200) )]
    min_interval: Duration,

    #[builder(setter(skip), default)]
    states: HashMap<String, DomainState>,
}
impl DomainRateLimiterBuilder {
    fn validate(&self) -> Result<(), String> {
        if let Some(m) = self.max_concurrency_per_domain
            && m == 0
        {
            return Err("max_concurrency_per_domain must be at least 1".to_string());
        }
        Ok(())
    }
}
impl DomainRateLimiter {
    pub async fn acquire(&mut self, domain: String) -> anyhow::Result<SemaphorePermit<'_>> {
        let state = self.states.entry(domain).or_insert_with(|| DomainState {
            sema: sync::Semaphore::new(self.max_concurrency_per_domain),
            last_request_started_at: sync::Mutex::new(Instant::now()),
        });
        let permit = state.sema.acquire().await?;
        let mut m = state.last_request_started_at.lock().await;
        if let Some(wait) = self.min_interval.checked_sub(m.elapsed()) {
            sleep(wait).await;
        }
        *m = Instant::now();
        Ok(permit)
    }
}
