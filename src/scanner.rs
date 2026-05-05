use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
    hash::Hash,
    path::Path,
    sync::Arc,
};

use anyhow::Result;
use tokio::{
    fs::{self},
    task,
};

use crate::handler::{Handler, Secret};

pub struct FileScanner<T, H>
where
    T: Borrow<Path> + Eq + Hash,
    H: Handler + Send + Sync + 'static,
{
    targets: Vec<T>,
    handler: Arc<H>,
}
impl<T, H> FileScanner<T, H>
where
    T: Borrow<Path> + Eq + Hash,
    H: Handler + Send + Sync,
{
    pub fn new(targets: Vec<T>, handler: H) -> Self {
        Self {
            targets,
            handler: Arc::new(handler),
        }
    }
    pub async fn scan(&self) -> Result<HashMap<&'_ T, HashSet<Secret>>> {
        let mut out: HashMap<&T, HashSet<Secret>> =
            HashMap::from_iter(self.targets.iter().map(|target| (target, HashSet::new())));
        for target in &self.targets {
            let content = fs::read_to_string(target.borrow()).await?;
            let handler = self.handler.clone();
            let secrets = task::spawn_blocking(move || handler.handle(&content)).await??;
            out.insert(target, HashSet::from_iter(secrets));
        }
        Ok(out)
    }
}
