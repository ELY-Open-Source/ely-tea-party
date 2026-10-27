use futures::channel::oneshot;
use gpui::{App, Global};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
};

mod jobs;
pub mod organize;
mod recall;
mod retention;
mod store;
#[cfg(test)]
mod tests;
pub mod window;
pub use store::{Item, Job, Overview, Source, Store};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Context {
    pub revision: u64,
    pub preferences: Vec<String>,
    pub recalled: Vec<Recall>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recall {
    pub id: String,
    pub text: String,
    pub reason: String,
}

type Work = Box<dyn FnOnce(&mut anyhow::Result<Store>) + Send>;

#[derive(Clone)]
pub struct Service {
    sender: mpsc::SyncSender<Work>,
    epoch: Arc<AtomicU64>,
}
impl Global for Service {}

impl Service {
    pub fn init(cx: &mut App) -> anyhow::Result<()> {
        let name = if cfg!(debug_assertions) || option_env!("ELY_DEV").is_some() {
            "memory-dev.sqlite"
        } else {
            "memory.sqlite"
        };
        let path = crate::settings::folder()?.join(name);
        let (sender, receiver) = mpsc::sync_channel::<Work>(32);
        std::thread::Builder::new()
            .name("whale-memory".into())
            .spawn(move || {
                let mut store = Store::open(path);
                if let Err(error) = &store {
                    log::error!("memory: cannot open database: {error:#}");
                }
                loop {
                    match receiver.recv_timeout(std::time::Duration::from_secs(60)) {
                        Ok(work) => work(&mut store),
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if let Ok(store) = &mut store
                                && let Err(error) = store.maintain(false)
                            {
                                log::warn!("memory: maintenance deferred: {error:#}");
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })?;
        cx.set_global(Self {
            sender,
            epoch: Arc::default(),
        });
        Ok(())
    }

    pub fn get(cx: &App) -> Self {
        cx.global::<Self>().clone()
    }
    pub fn revision(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }
    pub fn invalidate(&self) {
        self.epoch.fetch_add(1, Ordering::AcqRel);
    }

    pub fn request<T: Send + 'static>(
        &self,
        work: impl FnOnce(&mut Store) -> anyhow::Result<T> + Send + 'static,
    ) -> oneshot::Receiver<Result<T, String>> {
        let (tx, rx) = oneshot::channel();
        let job: Work = Box::new(move |store| {
            let result = match store {
                Ok(store) => work(store).map_err(|e| format!("memory: {e:#}")),
                Err(error) => Err(format!("memory: {error:#}")),
            };
            let _ = tx.send(result);
        });
        if self.sender.try_send(job).is_err() {
            log::warn!("memory: queue full or stopped; request refused");
        }
        rx
    }

    pub fn record(&self, folder: PathBuf, session: String, kind: String, text: String) {
        let text = bounded(&text, 2048);
        let epoch = self.epoch.clone();
        let revision = self.revision();
        let result = self.sender.try_send(Box::new(move |store| {
            if epoch.load(Ordering::Acquire) != revision {
                return;
            }
            if let Ok(store) = store
                && let Err(error) = store.record(&folder, &session, &kind, &text)
            {
                log::warn!("memory: event not recorded: {error:#}");
            }
        }));
        if result.is_err() {
            log::warn!("memory: event dropped because queue is full or stopped");
        }
    }
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

pub fn bounded(text: &str, bytes: usize) -> String {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
}

pub fn key(text: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

pub fn clean(text: &str) -> String {
    let text = text
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            ![
                "api_key",
                "api-key",
                "authorization:",
                "bearer ",
                "private key",
                "password=",
                "token=",
            ]
            .iter()
            .any(|pattern| lower.contains(pattern))
        })
        .collect::<Vec<_>>()
        .join("\n");
    bounded(text.trim(), 2048)
}

pub fn task_key(text: &str) -> String {
    key(&clean(text).split_whitespace().collect::<Vec<_>>().join(" "))
}
pub fn checkout(folder: &std::path::Path) -> String {
    key(&folder
        .canonicalize()
        .unwrap_or_else(|_| folder.to_path_buf())
        .to_string_lossy())
}
