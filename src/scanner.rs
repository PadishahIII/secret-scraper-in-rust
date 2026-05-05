use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
    error::Error,
    hash::Hash,
    path::Path,
};

use tokio::fs::{self};

use crate::handler::{Handler, Secret};

pub struct FileScanner<T, H>
where
    T: Borrow<Path> + Eq + Hash,
    H: Handler,
{
    targets: Vec<T>,
    handler: H,
}
impl<T, H> FileScanner<T, H>
where
    T: Borrow<Path> + Eq + Hash,
    H: Handler,
{
    pub fn new(targets: Vec<T>, handler: H) -> Self {
        Self { targets, handler }
    }
    pub async fn scan(&self) -> Result<HashMap<&'_ T, HashSet<Secret<'_>>>, Box<dyn Error>> {
        let mut out: HashMap<&T, HashSet<Secret>> =
            HashMap::from_iter(self.targets.iter().map(|target| (target, HashSet::new())));
        for target in &self.targets {
            let content = fs::read_to_string(target.borrow()).await?;
            let secrets = self.handler.handle(&content)?; // TODO: should be nonblocking
            out.insert(target, HashSet::from_iter(secrets));
        }
        Ok(out)
    }
}
