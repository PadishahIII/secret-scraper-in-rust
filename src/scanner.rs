//! Local file scanning engine.

use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
    hash::Hash,
    io,
    path::Path,
    sync::Arc,
};

use anyhow::Result;
use tokio::{fs, task};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::handler::{Handler, Secret};

/// Scans a set of local file targets with a secret [`Handler`].
pub struct FileScanner<T, H>
where
    T: Borrow<Path> + Eq + Hash,
    H: Handler + Send + Sync + 'static,
{
    targets: Vec<T>,
    handler: Arc<H>,
    shutdown: CancellationToken,
}
impl<T, H> FileScanner<T, H>
where
    T: Borrow<Path> + Eq + Hash,
    H: Handler + Send + Sync,
{
    /// Create a scanner for `targets` using `handler`.
    #[allow(dead_code)]
    pub fn new(targets: Vec<T>, handler: H) -> Self {
        Self::with_shutdown(targets, handler, CancellationToken::new())
    }

    /// Create a scanner for `targets` using `handler` and a shutdown token.
    pub fn with_shutdown(targets: Vec<T>, handler: H, shutdown: CancellationToken) -> Self {
        Self {
            targets,
            handler: Arc::new(handler),
            shutdown,
        }
    }
    /// Scan all targets and return detected secrets keyed by target.
    pub async fn scan(&self) -> Result<HashMap<&'_ T, HashSet<Secret>>> {
        let mut out: HashMap<&T, HashSet<Secret>> = HashMap::new();
        for target in &self.targets {
            if self.shutdown.is_cancelled() {
                break;
            }
            match fs::read_to_string(target.borrow()).await {
                Ok(content) => {
                    let handler = self.handler.clone();
                    let secrets = task::spawn_blocking(move || handler.handle(&content)).await??;
                    out.insert(target, HashSet::from_iter(secrets));
                }
                Err(err) if err.kind() == io::ErrorKind::InvalidData => {
                    info!(
                        "ignore {:?} since it's not UTF-8 data",
                        target.borrow().as_os_str()
                    )
                }
                Err(err) => return Err(err.into()),
            };
        }
        Ok(out)
    }
}
