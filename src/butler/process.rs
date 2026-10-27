use std::{
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use crate::term::snapshot::Snapshot;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Purpose {
    #[default]
    Explain,
    Idea,
    Remember,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub version: u8,
    pub purpose: Purpose,
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub language: String,
    pub persona: String,
    pub status: super::tasks::Status,
    pub snapshots: Vec<Snapshot>,
    pub memory: crate::memory::Context,
    pub consolidation: Option<crate::memory::Job>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    #[serde(skip)]
    Prepared {
        key: String,
    },
    #[serde(skip)]
    Cached {
        text: String,
    },
    Reset,
    Delta {
        text: String,
    },
    Done {
        text: String,
        sources: Vec<String>,
        #[serde(default)]
        tokens: Option<u64>,
    },
    Error {
        text: String,
    },
}

pub struct Cancellation(pub Arc<AtomicBool>);
impl Default for Cancellation {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
}
impl Drop for Cancellation {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn run(request: Request, cancelled: Arc<AtomicBool>, events: Sender<Event>) {
    if let Err(error) = execute(request, cancelled, &events) {
        let _ = events.send(Event::Error {
            text: format!("butler: {error:#}"),
        });
    }
}

fn assets() -> anyhow::Result<PathBuf> {
    let exe = std::env::current_exe().context("cannot locate application")?;
    let dir = exe.parent().context("application has no directory")?;
    let mut candidates = vec![dir.join("butler"), dir.join("../Resources/butler")];
    #[cfg(target_os = "linux")]
    candidates.push(dir.join("../lib/ely-tea-party/butler"));
    if cfg!(debug_assertions) {
        candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/butler"));
    }
    let node = if cfg!(windows) { "node.exe" } else { "node" };
    candidates
        .into_iter()
        .find(|dir| dir.join("main.mjs").is_file() && dir.join(node).is_file())
        .context(
            "Pi runtime is missing; development builds need: cd butler && npm ci && npm run build",
        )
}

fn execute(
    request: Request,
    cancelled: Arc<AtomicBool>,
    events: &Sender<Event>,
) -> anyhow::Result<()> {
    let dir = assets()?;
    if cancelled.load(Ordering::Acquire) {
        return Ok(());
    }
    let mut command = Command::new(dir.join(if cfg!(windows) { "node.exe" } else { "node" }));
    command
        .arg(dir.join("main.mjs"))
        .env_remove("NODE_OPTIONS")
        .env_remove("NODE_PATH")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let child = Running(command.spawn().context("cannot start Pi runtime")?);
    exchange(child, request, cancelled, events)
}

fn exchange(
    mut child: Running,
    request: Request,
    cancelled: Arc<AtomicBool>,
    events: &Sender<Event>,
) -> anyhow::Result<()> {
    log::info!("butler: Pi runtime {} starts", child.0.id());
    let mut stdin = child.0.stdin.take().context("Pi stdin is missing")?;
    let stdout = child.0.stdout.take().context("Pi stdout is missing")?;
    let payload = serde_json::to_vec(&request).context("cannot encode snapshot request")?;
    anyhow::ensure!(
        payload.len() <= 2 * 1024 * 1024,
        "snapshot request exceeds 2 MiB"
    );
    let writer = thread::spawn(move || stdin.write_all(&payload));
    let (tx, rx) = mpsc::channel();
    let reader = thread::spawn(move || read_events(stdout, tx));
    let started = Instant::now();
    let mut done = false;
    let outcome = (|| {
        loop {
            if cancelled.load(Ordering::Acquire) {
                log::info!("butler: run cancelled");
                return Ok(());
            }
            anyhow::ensure!(
                started.elapsed() < Duration::from_secs(120),
                "run timed out after 120 seconds"
            );
            match rx.recv_timeout(Duration::from_millis(25)) {
                Ok(Ok(mut event)) => {
                    match &mut event {
                        Event::Delta { text }
                        | Event::Done { text, .. }
                        | Event::Error { text } => {
                            *text = text.replace(&request.api_key, "[redacted]");
                        }
                        _ => {}
                    }
                    done |= matches!(event, Event::Done { .. } | Event::Error { .. });
                    if events.send(event).is_err() {
                        return Ok(());
                    }
                }
                Ok(Err(error)) => return Err(error),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    anyhow::ensure!(done, "Pi exited without a report");
                    return Ok(());
                }
            }
        }
    })();
    let _ = child.0.kill();
    let status = child.0.wait();
    let _ = writer.join();
    let _ = reader.join();
    log::info!("butler: Pi runtime exits ({status:?})");
    outcome
}

fn read_events(stdout: impl Read, tx: Sender<anyhow::Result<Event>>) {
    let mut reader = BufReader::new(stdout);
    let mut total = 0;
    loop {
        let mut line = Vec::new();
        let got = reader.by_ref().take(65_537).read_until(b'\n', &mut line);
        let parsed = (|| {
            let count = got.context("cannot read Pi events")?;
            if count == 0 {
                return Ok(None);
            }
            total += count;
            anyhow::ensure!(
                count <= 65_536 && total <= 512_000,
                "Pi output exceeds its limit"
            );
            Ok(Some(
                serde_json::from_slice::<Event>(&line).context("invalid Pi event")?,
            ))
        })();
        match parsed {
            Ok(Some(event)) => {
                if tx.send(Ok(event)).is_err() {
                    break;
                }
            }
            Ok(None) => break,
            Err(error) => {
                let _ = tx.send(Err(error));
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversized_and_invalid_events() {
        for input in [vec![b'x'; 70_000], b"{\"type\":\"unknown\"}\n".to_vec()] {
            let (tx, rx) = mpsc::channel();
            read_events(&input[..], tx);
            assert!(rx.recv().unwrap().is_err());
        }
    }

    #[test]
    fn cancellation_survives_its_owner() {
        let owner = Cancellation::default();
        let worker = owner.0.clone();
        drop(owner);
        assert!(worker.load(Ordering::Acquire));
    }

    fn request() -> Request {
        Request {
            version: 1,
            purpose: Purpose::Explain,
            memory: Default::default(),
            consolidation: None,
            provider: "deepseek".into(),
            model: "missing-model-for-test".into(),
            api_key: "test-only-key".into(),
            language: "zh-CN".into(),
            persona: String::new(),
            status: super::super::tasks::Status::Unknown,
            snapshots: vec![Snapshot {
                id: "1".into(),
                title: "Test".into(),
                folder: "/tmp".into(),
                url: "ely://session/12345678-1234-4234-8234-123456789abc".into(),
                captured_at: "now".into(),
                alternate_screen: false,
                truncated: false,
                lines: vec!["test".into()],
            }],
        }
    }

    #[test]
    #[ignore = "requires npm --prefix butler run build"]
    fn bundled_pi_validates_the_model_without_contacting_a_provider() {
        let (tx, rx) = mpsc::channel();
        execute(request(), Arc::new(AtomicBool::new(false)), &tx).unwrap();
        assert!(rx.try_iter().any(
            |event| matches!(event, Event::Error { text } if text.contains("not in Pi's catalog"))
        ));
    }

    #[test]
    #[cfg(unix)]
    fn cancelling_a_silent_child_reaps_it_promptly() {
        let child = Command::new("/bin/sleep")
            .arg("30")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let pid = child.id();
        let cancelled = Arc::new(AtomicBool::new(false));
        let signal = cancelled.clone();
        let cancel = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            signal.store(true, Ordering::Release);
        });
        let (tx, _rx) = mpsc::channel();
        let start = Instant::now();
        exchange(Running(child), request(), cancelled, &tx).unwrap();
        cancel.join().unwrap();
        assert!(start.elapsed() < Duration::from_secs(2));
        assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1, "child was reaped");
    }
}
