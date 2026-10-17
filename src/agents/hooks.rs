//! Agents' hooks report to the app through a socket each session names in its environment.

use std::{
    io::{Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    sync::OnceLock,
    time::Duration,
};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use serde::{Deserialize, Serialize};

use super::{Kind, Status};

mod payload;

/// The session a hook runs in.
pub const SESSION: &str = "ELY_SESSION";
/// The app's socket.
pub const SOCKET: &str = "ELY_SOCKET";
/// This executable, which hooks run as `$ELY_HOOK hook AGENT`.
pub const HOOK: &str = "ELY_HOOK";

static SOCKET_PATH: OnceLock<PathBuf> = OnceLock::new();

/// One hook's word: which session, which agent, which event, and what it touches.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub session: u64,
    pub agent: String,
    pub event: String,
    /// The tool the event names, as `Read` or `Bash`.
    #[serde(default)]
    pub tool: Option<String>,
    /// What it works on or says: a file's name, a command, a pattern, a message.
    #[serde(default)]
    pub detail: Option<String>,
    /// The agent's conversation, which resumes it.
    #[serde(default)]
    pub conversation: Option<String>,
}

/// What an event means for the agent it comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    To(Status),
    Ended,
}

impl Report {
    pub fn kind(&self) -> Option<Kind> {
        Kind::named(&self.agent)
    }

    /// Claude Code's hook events, which Codex's notify and others borrow.
    pub fn change(&self) -> Option<Change> {
        Some(match self.event.as_str() {
            "SessionStart" | "Stop" | "StopFailure" | "agent-turn-complete" => {
                Change::To(Status::Idle)
            }
            "UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "PostToolUseFailure"
            | "PreCompact" | "SubagentStart" | "SubagentStop" => Change::To(Status::Working),
            "Notification" | "PermissionRequest" => Change::To(Status::NeedsInput),
            "SessionEnd" => Change::Ended,
            _ => return None,
        })
    }
}

/// Opens the app's socket; its reports come out of the receiver, in order.
pub fn listen() -> anyhow::Result<UnboundedReceiver<Report>> {
    let folder = dirs::runtime_dir().unwrap_or_else(std::env::temp_dir);
    let path = folder.join(format!("ely-tea-party-{}.sock", std::process::id()));
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|error| anyhow::anyhow!("hooks: cannot clear {}: {error}", path.display()))?;
    }
    let listener = UnixListener::bind(&path)
        .map_err(|error| anyhow::anyhow!("hooks: cannot open {}: {error}", path.display()))?;
    log::info!("hooks: listening on {}", path.display());
    SOCKET_PATH
        .set(path)
        .map_err(|_| anyhow::anyhow!("hooks: the socket opened twice"))?;
    let (sender, reports) = unbounded();
    std::thread::Builder::new()
        .name("hooks".into())
        .spawn(move || serve(listener, sender))?;
    Ok(reports)
}

fn serve(listener: UnixListener, sender: UnboundedSender<Report>) {
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                log::error!("hooks: a connection failed: {error}");
                continue;
            }
        };
        let mut text = String::new();
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        if let Err(error) = stream.read_to_string(&mut text) {
            log::error!("hooks: a report did not arrive: {error}");
            continue;
        }
        match serde_json::from_str::<Report>(&text) {
            Ok(report) => {
                log::debug!("hooks: {report:?}");
                if sender.unbounded_send(report).is_err() {
                    log::info!("hooks: no window hears reports; the socket closes");
                    return;
                }
            }
            Err(error) => log::error!("hooks: a report that does not parse: {error}"),
        }
    }
}

/// Removes the socket as the app quits.
pub fn close() {
    if let Some(path) = SOCKET_PATH.get()
        && let Err(error) = std::fs::remove_file(path)
    {
        log::error!("hooks: cannot remove {}: {error}", path.display());
    }
}

/// The variables that let session `id`'s hooks find the app.
pub fn env(id: u64) -> anyhow::Result<Vec<(String, String)>> {
    let socket = SOCKET_PATH
        .get()
        .ok_or_else(|| anyhow::anyhow!("hooks: a session started before the socket opened"))?;
    let exe = std::env::current_exe()
        .map_err(|error| anyhow::anyhow!("hooks: this executable has no path: {error}"))?;
    Ok(vec![
        (SESSION.into(), id.to_string()),
        (SOCKET.into(), socket.display().to_string()),
        (HOOK.into(), exe.display().to_string()),
    ])
}

/// `ely-tea-party hook AGENT [EVENT] [JSON]`: silent and always succeeding, so it never stops the agent.
pub fn run(args: &[String]) {
    if let Err(error) = send(args) {
        log::debug!("hook: {error:#}");
    }
}

/// `hook AGENT [EVENT] [JSON]`: the payload comes as an argument, as Codex's notify passes it, else on stdin.
fn send(args: &[String]) -> anyhow::Result<()> {
    let agent = args
        .first()
        .ok_or_else(|| anyhow::anyhow!("no agent named"))?;
    // Grok runs Claude Code's hooks too; its own hooks speak for it.
    if agent == "claude"
        && std::env::var_os("GROK_HOOK_EVENT").is_some_and(|event| !event.is_empty())
    {
        return Ok(());
    }
    let session = std::env::var(SESSION)?.parse()?;
    let socket = std::env::var(SOCKET)?;
    let rest = &args[1..];
    let named = rest.first().filter(|arg| !arg.starts_with('{'));
    let payload = match rest.iter().find(|arg| arg.starts_with('{')) {
        Some(json) => json.clone(),
        None => {
            let mut text = String::new();
            std::io::stdin().read_to_string(&mut text)?;
            text
        }
    };
    let payload: serde_json::Value = match payload.trim() {
        "" => serde_json::json!({}),
        text => serde_json::from_str(text)?,
    };
    let Some(said) = payload::read(agent, named.map(String::as_str), &payload) else {
        return Ok(());
    };
    let report = Report {
        session,
        agent: agent.clone(),
        event: said.event,
        tool: said.tool,
        detail: said.detail,
        conversation: said.conversation,
    };
    let mut stream = UnixStream::connect(socket)?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    stream.write_all(serde_json::to_string(&report)?.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(event: &str) -> Option<Change> {
        let report = Report {
            session: 1,
            agent: "claude".into(),
            event: event.into(),
            tool: None,
            detail: None,
            conversation: None,
        };
        report.change()
    }

    #[test]
    fn events_move_the_agent() {
        assert_eq!(change("PreToolUse"), Some(Change::To(Status::Working)));
        assert_eq!(change("Notification"), Some(Change::To(Status::NeedsInput)));
        assert_eq!(change("Stop"), Some(Change::To(Status::Idle)));
        assert_eq!(
            change("agent-turn-complete"),
            Some(Change::To(Status::Idle))
        );
        assert_eq!(change("SessionEnd"), Some(Change::Ended));
        assert_eq!(change("Whatever"), None);
    }

    #[test]
    fn a_report_crosses_the_socket() {
        let reports = listen().expect("a socket");
        let env = env(7).expect("its variables");
        let socket = &env
            .iter()
            .find(|(name, _)| name == SOCKET)
            .expect("the socket")
            .1;
        let mut stream = UnixStream::connect(socket).expect("a connection");
        stream
            .write_all(br#"{"session":7,"agent":"codex","event":"Stop"}"#)
            .expect("written");
        drop(stream);
        let report = futures::executor::block_on(futures::StreamExt::next(&mut { reports }));
        assert_eq!(report.map(|report| report.session), Some(7));
        close();
    }
}
