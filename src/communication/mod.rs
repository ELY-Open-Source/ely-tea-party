use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

use anyhow::Context as _;
use futures::StreamExt;
use gpui::{App, Global, Task};

mod cli;
mod mail;
pub mod path;
pub mod protocol;
mod router;
pub mod skills;
mod transport;
pub mod ui;

pub use cli::command;

struct Connection {
    endpoint: String,
    tokens: Arc<Mutex<HashMap<u64, String>>>,
}
static CONNECTION: OnceLock<Connection> = OnceLock::new();

struct Service {
    _server: transport::Server,
    _task: Task<()>,
}
impl Global for Service {}

pub fn init(cx: &mut App) -> anyhow::Result<()> {
    ui::init(cx);
    let dir = dirs::data_local_dir()
        .context("messages: no local data directory")?
        .join("ely-tea-party");
    let mail = mail::Mail::open(&dir.join("messages.sqlite3"))?;
    let (server, mut receiver) = transport::listen()?;
    let tokens = Arc::new(Mutex::new(HashMap::new()));
    CONNECTION
        .set(Connection {
            endpoint: server.endpoint.clone(),
            tokens: tokens.clone(),
        })
        .map_err(|_| anyhow::anyhow!("messages: service initialized twice"))?;
    let router = Arc::new(Mutex::new(router::Router::new(mail)));
    let task = cx.spawn(async move |cx| {
        while let Some(job) = receiver.next().await {
            let snapshots = cx.update(|cx| snapshots(cx));
            let router = router.clone();
            let tokens = tokens.clone();
            let response = cx
                .background_executor()
                .spawn(async move {
                    let result = (|| {
                        let mut router = router
                            .lock()
                            .map_err(|_| anyhow::anyhow!("messages: router failed"))?;
                        router.refresh(snapshots)?;
                        let tokens = tokens
                            .lock()
                            .map_err(|_| anyhow::anyhow!("messages: credential registry failed"))?;
                        router.execute(job.request, &tokens)
                    })();
                    protocol::Response::from_result(result)
                })
                .await;
            let _ = job.reply.send(response);
        }
    });
    cx.set_global(Service {
        _server: server,
        _task: task,
    });
    cx.on_app_quit(|cx| {
        cx.global::<Service>()._server.close();
        async {}
    })
    .detach();
    log::info!(
        "messages: local mailbox service started, protocol {}",
        protocol::VERSION
    );
    Ok(())
}

pub fn env(id: u64, drawer: bool) -> anyhow::Result<Vec<(String, String)>> {
    let connection = CONNECTION
        .get()
        .context("messages: session started before the service")?;
    let token = if drawer {
        String::new()
    } else {
        connection
            .tokens
            .lock()
            .map_err(|_| anyhow::anyhow!("messages: credential registry failed"))?
            .entry(id)
            .or_insert_with(|| uuid::Uuid::new_v4().to_string())
            .clone()
    };
    Ok(vec![
        ("ELY_MSG_ENDPOINT".into(), connection.endpoint.clone()),
        ("ELY_MSG_TOKEN".into(), token),
        ("ELY_SESSION".into(), id.to_string()),
    ])
}

fn snapshots(cx: &App) -> Vec<router::Snapshot> {
    let mut all = Vec::new();
    for window in cx
        .windows()
        .into_iter()
        .filter_map(|window| window.downcast::<crate::workspace::Workspace>())
    {
        let workspace = match window.read(cx) {
            Ok(workspace) => workspace,
            Err(error) => {
                log::debug!("messages: unavailable window: {error}");
                continue;
            }
        };
        for session in workspace.sessions() {
            if session.terminal.read(cx).ended().is_some() {
                continue;
            }
            let agent = session.agent.as_ref();
            let identity = agent.map_or_else(
                || "shell".into(),
                |agent| {
                    format!(
                        "{}:{}",
                        agent.instance,
                        agent.conversation.as_deref().unwrap_or_default()
                    )
                },
            );
            let name = agent.map_or_else(
                || "Terminal".into(),
                |agent| {
                    let name = crate::agents::icons::name(agent.kind);
                    agent
                        .program
                        .as_ref()
                        .map_or_else(|| name.into(), |program| format!("{name} ({program})"))
                },
            );
            all.push(router::Snapshot {
                session: session.id,
                identity,
                agent: name,
                title: crate::sessions::compact::title(session),
                folder: session.folder.to_string_lossy().into_owned(),
                state: format!("{:?}", session.task_state(cx)).to_lowercase(),
            });
        }
    }
    all
}
