use std::{
    hash::{DefaultHasher, Hash, Hasher},
    sync::{atomic::Ordering, mpsc},
    time::{Duration, Instant},
};

use gpui::{App, BorrowAppContext, Context, Subscription, Task, Window};

use super::Workspace;
use crate::{
    butler::{
        Briefing, Credentials, cache,
        companion::{Budget, Cadence},
        process::{self, Cancellation, Event, Purpose, Request},
        tasks::{self, Status},
    },
    settings::{Mascot, SettingsStore},
};

mod bubble;

#[derive(Default)]
pub(super) struct Companion {
    cadence: Cadence,
    context: Option<String>,
    pub reply: Briefing,
    expires: Option<Instant>,
    muted: bool,
    pub suggestion: Option<String>,
    timer: Option<Task<()>>,
    watch: Vec<Subscription>,
}

impl Workspace {
    pub(super) fn watch_companion(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.default_global::<Budget>();
        self.companion.watch = vec![
            cx.observe_global::<crate::pet::Touched>(|this, cx| this.quiet_companion(cx)),
            cx.observe_window_activation(window, |this, _, cx| this.quiet_companion(cx)),
            cx.observe_window_visibility(window, |this, _, _, cx| this.quiet_companion(cx)),
        ];
        self.companion.timer = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if cx
                    .update(|window, cx| {
                        this.update(cx, |this, cx| this.companion_tick(window, cx))
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
    }

    pub(super) fn quiet_companion(&mut self, cx: &mut Context<Self>) {
        let reply = &mut self.companion.reply;
        if reply.visible || reply.running.is_some() {
            log::debug!("companion: interrupted; hiding idea and cancelling request");
            reply.cancel();
            reply.visible = false;
            reply.text.clear();
            cx.notify();
        }
        self.companion.expires = None;
        self.companion.context = None;
        self.companion.cadence.reset();
    }

    fn companion_context(&self, window: &Window, cx: &App) -> Option<String> {
        let settings = SettingsStore::get(cx);
        if !settings.butler.companion
            || !settings.ely
            || settings.mascot != Mascot::Whale
            || !window.is_window_active()
            || !window.is_visible()
            || self.party
            || self.butler.visible
            || self.butler.running.is_some()
            || self.companion.muted
        {
            return None;
        }
        let session = self.sessions.get(self.active)?;
        if session.agent.is_none()
            || tasks::observed(session, cx) != Status::Working
            || session.drawer.as_ref().is_some_and(|drawer| drawer.open)
            || self.sessions.iter().any(|session| {
                matches!(
                    tasks::observed(session, cx),
                    Status::Error | Status::Waiting
                )
            })
        {
            return None;
        }
        Some(format!(
            "{}\n{}",
            session.link_id.url(),
            session.folder.display()
        ))
    }

    fn companion_tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let now = Instant::now();
        if window.is_window_active()
            && window.is_visible()
            && let Some(session) = self.sessions.get(self.active)
        {
            crate::memory::organize::poll(
                session.folder.clone(),
                SettingsStore::get(cx).butler.clone(),
                SettingsStore::get(cx).language.tag().into(),
                cx,
            );
        }
        let context = self.companion_context(window, cx);
        if context != self.companion.context {
            self.quiet_companion(cx);
            self.companion.context = context.clone();
        }
        if context.is_none() {
            return;
        }
        if self.companion.expires.is_some_and(|at| now >= at) {
            self.companion.reply.visible = false;
            self.companion.expires = None;
            cx.notify();
        }
        let due = self
            .companion
            .cadence
            .observe(now, context.clone(), crate::pet::touched(cx));
        if due
            && cx.global::<Budget>().available(now)
            && self.companion.reply.running.is_none()
            && !self.companion.reply.visible
        {
            self.start_companion(context.expect("eligible context"), window, cx);
        }
    }

    fn start_companion(&mut self, context: String, window: &mut Window, cx: &mut Context<Self>) {
        let config = SettingsStore::get(cx).butler.clone();
        cx.default_global::<Credentials>();
        let key = match config.api_key(cx.global::<Credentials>()) {
            Ok(key) => key,
            Err(_) => {
                log::debug!("companion: automatic idea skipped; connection is not configured");
                return;
            }
        };
        let session = &self.sessions[self.active];
        let source = session.terminal.read(cx).snapshot_source(
            session.id.to_string(),
            session.named().chars().take(80).collect(),
            session.folder.to_string_lossy().chars().take(256).collect(),
            session.link_id.url(),
        );
        let prompt = session
            .log
            .entries
            .iter()
            .rev()
            .find_map(|entry| match &entry.step {
                crate::agents::timeline::Step::Prompt(Some(prompt)) => Some(prompt.clone()),
                _ => None,
            });
        let memory_service = crate::memory::Service::get(cx);
        let memory_revision = memory_service.revision();
        let memory_enabled = config.memory;
        let folder = session.folder.clone();
        let session_link = session.link_id.url();
        let task = prompt.clone().unwrap_or_default();
        let read_folder = folder.clone();
        let read_session = session_link.clone();
        let read_task = task.clone();
        let read_service = memory_service.clone();
        let language = SettingsStore::get(cx).language.tag().to_string();
        let cancel = Cancellation::default();
        let flag = cancel.0.clone();
        self.companion.reply.running = Some(cancel);
        let prepared = cx.background_executor().spawn(async move {
            let mut snapshot = source.capture();
            cache::bound(&mut snapshot, 20, 1600);
            if let Some(prompt) = prompt {
                snapshot
                    .lines
                    .insert(0, format!("User task: {}", tasks::excerpt(&prompt)));
            }
            cache::bound(&mut snapshot, 24, 2400);
            let memory = if memory_enabled {
                match read_service
                    .request(move |store| {
                        store.recall(&read_folder, &read_session, &read_task, memory_revision)
                    })
                    .await
                {
                    Ok(Ok(memory)) => memory,
                    other => {
                        log::warn!("memory: recall unavailable: {other:?}");
                        Default::default()
                    }
                }
            } else {
                Default::default()
            };
            Request {
                version: 1,
                memory,
                consolidation: None,
                purpose: Purpose::Idea,
                provider: config.provider,
                model: config.model,
                api_key: key,
                language,
                persona: config.persona.chars().take(500).collect(),
                status: Status::Working,
                snapshots: vec![snapshot],
            }
        });
        self.companion.reply.task = Some(cx.spawn_in(window, async move |this, cx| {
            let request = prepared.await;
            let recalled = request.memory.clone();
            let mut hash = DefaultHasher::new();
            cache::key(&request).hash(&mut hash);
            let fingerprint = hash.finish();
            let start = cx
                .update(|window, cx| {
                    this.update(cx, |this, cx| {
                        if (memory_enabled && memory_service.revision() != memory_revision)
                            || flag.load(Ordering::Acquire)
                            || this.companion_context(window, cx).as_ref() != Some(&context)
                        {
                            this.quiet_companion(cx);
                            return false;
                        }
                        if request.snapshots[0]
                            .lines
                            .iter()
                            .all(|line| line.trim().is_empty())
                            || !cx.update_global::<Budget, _>(|budget, _| {
                                budget.reserve(Instant::now(), fingerprint)
                            })
                        {
                            this.companion.reply.running = None;
                            return false;
                        }
                        log::info!("companion: one bounded idea request starts");
                        true
                    })
                })
                .ok()
                .and_then(Result::ok)
                .unwrap_or(false);
            if !start {
                return;
            }
            let (tx, rx) = mpsc::channel();
            let store_service = memory_service.clone();
            let (stored_tx, stored_rx) = mpsc::channel();
            let _worker = cx.background_executor().spawn(async move {
                let (model_tx, model_rx) = mpsc::channel();
                process::run(request, flag, model_tx);
                for event in model_rx.try_iter().collect::<Vec<_>>() {
                    if let Event::Done { text, .. } = &event
                        && memory_enabled
                    {
                        if store_service.revision() != memory_revision {
                            break;
                        }
                        let text = text.clone();
                        let write_service = store_service.clone();
                        let saved = store_service
                            .request(move |store| {
                                anyhow::ensure!(
                                    write_service.revision() == memory_revision,
                                    "memory changed before idea was stored"
                                );
                                let id = store.suggestion(&folder, &text)?;
                                if id.is_some() {
                                    store.used(&session_link, &task, &recalled)?;
                                }
                                Ok(id)
                            })
                            .await;
                        match saved {
                            Ok(Ok(Some(id))) => {
                                let _ = stored_tx.send(id);
                            }
                            Ok(Ok(None)) => {
                                let _ = tx.send(Event::Error {
                                    text: "duplicate idea suppressed".into(),
                                });
                                break;
                            }
                            error => {
                                log::warn!("memory: idea not stored: {error:?}");
                                let _ = tx.send(Event::Error {
                                    text: "memory unavailable".into(),
                                });
                                break;
                            }
                        }
                        let _ = tx.send(event);
                        break;
                    }
                    let _ = tx.send(event);
                }
            });
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let finished = cx
                    .update(|window, cx| {
                        this.update(cx, |this, cx| {
                            if (memory_enabled && memory_service.revision() != memory_revision)
                                || this.companion_context(window, cx).as_ref() != Some(&context)
                            {
                                this.quiet_companion(cx);
                                return true;
                            }
                            for event in rx.try_iter() {
                                match event {
                                    Event::Done { text, tokens, .. } => {
                                        this.companion.suggestion = stored_rx.try_recv().ok();
                                        let reply = &mut this.companion.reply;
                                        reply.running = None;
                                        reply.text = text.chars().take(240).collect();
                                        reply.tokens = tokens;
                                        reply.visible = !reply.text.trim().is_empty();
                                        this.companion.expires =
                                            Some(Instant::now() + Duration::from_secs(25));
                                        log::info!(
                                            "companion: idea shown; reported tokens {tokens:?}"
                                        );
                                        cx.notify();
                                        return true;
                                    }
                                    Event::Error { .. } => {
                                        this.companion.reply.running = None;
                                        log::info!("companion: idea failed; cooldown retained");
                                        return true;
                                    }
                                    _ => {}
                                }
                            }
                            false
                        })
                    })
                    .ok()
                    .and_then(Result::ok)
                    .unwrap_or(true);
                if finished {
                    break;
                }
            }
        }));
    }
}
