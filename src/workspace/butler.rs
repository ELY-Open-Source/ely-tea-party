use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use gpui::{Context, Window};

use super::Workspace;
use crate::{
    butler::{
        Credentials, cache,
        placement::Anchor,
        process::{self, Cancellation, Event, Request},
        tasks,
    },
    settings::SettingsStore,
};

mod bubble;
mod cards;

impl Workspace {
    fn open_session_link(
        &mut self,
        id: crate::sessions::LinkId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(index) = self.sessions.iter().position(|s| s.link_id == id) {
            log::debug!("butler: opening session link {}", id.url());
            self.select(index, window, cx);
        } else {
            log::debug!("butler: linked session has closed");
        }
    }

    pub fn dismiss_butler(&mut self, cx: &mut Context<Self>) {
        if self.butler.visible {
            log::debug!("butler: bubble dismissed by outside click");
            self.butler.cancel();
            self.butler.visible = false;
            cx.notify();
        }
    }

    pub fn move_butler(&mut self, anchor: Anchor, cx: &mut Context<Self>) {
        if self.butler.visible && self.butler.anchor != anchor {
            log::debug!("butler: bubble follows mascot at {anchor:?}");
            self.butler.anchor = anchor;
            cx.notify();
        }
    }

    pub fn show_butler(&mut self, anchor: Anchor, window: &mut Window, cx: &mut Context<Self>) {
        if self.butler.visible {
            self.move_butler(anchor, cx);
            return;
        }
        let first = self
            .sessions
            .iter()
            .filter_map(|session| {
                let status = tasks::observed(session, cx);
                matches!(status, tasks::Status::Error | tasks::Status::Waiting)
                    .then_some((status, session.link_id))
            })
            .min_by_key(|(status, _)| *status)
            .map(|(_, id)| id);
        if let Some(id) = first {
            self.open_session_link(id, window, cx);
        }
        self.butler.visible = true;
        self.butler.generation = self.butler.generation.wrapping_add(1);
        self.butler.anchor = anchor;
        self.butler.cancel();
        self.butler.error = None;
        self.butler.text.clear();
        self.butler.target = None;
        self.butler.expanded = None;
        log::info!("butler: local task cards opened without a model request");
        cx.notify();
    }

    fn explain_task(&mut self, id: crate::sessions::LinkId, cx: &mut Context<Self>) {
        if !self.butler.visible
            || !SettingsStore::get(cx).butler.enabled
            || self.butler.running.is_some()
        {
            return;
        }
        let Some(session) = self.sessions.iter().find(|session| session.link_id == id) else {
            return;
        };
        let source = session.terminal.read(cx).snapshot_source(
            session.id.to_string(),
            session.named().chars().take(160).collect(),
            session.folder.to_string_lossy().chars().take(512).collect(),
            session.link_id.url(),
        );
        let status = tasks::observed(session, cx);
        self.butler.cancel();
        self.butler.target = Some(id);
        self.butler.expanded = Some(id);
        self.butler.cached = false;
        self.butler.tokens = None;
        self.butler.error = None;
        self.butler.text.clear();
        let config = SettingsStore::get(cx).butler.clone();
        let language = SettingsStore::get(cx).language.tag().to_string();
        cx.default_global::<Credentials>();
        let key = match config.api_key(cx.global::<Credentials>()) {
            Ok(key) => key,
            Err(error) => {
                self.butler.error = Some(error.to_string());
                return cx.notify();
            }
        };
        let cache = self.butler.cache.clone();
        let cancel = Cancellation::default();
        let flag = cancel.0.clone();
        let (tx, rx) = mpsc::channel();
        self.butler.running = Some(cancel);
        let worker = cx.background_executor().spawn(async move {
            let mut snapshot = source.capture();
            cache::compact(&mut snapshot);
            if snapshot.lines.iter().all(|line| line.trim().is_empty()) {
                let _ = tx.send(Event::Error {
                    text: if language == "zh-CN" {
                        "没有可解读的终端文本，未调用模型。".into()
                    } else {
                        "No terminal text to explain; no model request was sent.".into()
                    },
                });
                return;
            }
            let request = Request {
                version: 1,
                provider: config.provider,
                model: config.model,
                api_key: key,
                language,
                persona: config.persona,
                status,
                snapshots: vec![snapshot],
            };
            let key = cache::key(&request);
            if let Some(text) = cache.get(&key, Instant::now()) {
                let _ = tx.send(Event::Cached { text });
                return;
            }
            let _ = tx.send(Event::Prepared { key });
            process::run(request, flag, tx);
        });
        self.butler.task = Some(cx.spawn(async move |this, cx| {
            let _worker = worker;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                let finished = this.update(cx, |this, cx| {
                    let mut finished = false;
                    let mut changed = false;
                    for event in rx.try_iter() {
                        changed = true;
                        finished |= this.butler.apply(event);
                        if finished {
                            break;
                        }
                    }
                    if changed {
                        cx.notify();
                    }
                    finished
                });
                if finished.unwrap_or(true) {
                    break;
                }
            }
        }));
        cx.notify();
    }
}
