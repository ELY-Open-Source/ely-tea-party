use std::time::Instant;

use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    i18n::text,
    theme::{ActiveTheme, ControlSize},
};
use gpui::{Context, Div, FontWeight, Window, div, prelude::*, rems};

use crate::{
    agents::{
        activity::Doing,
        timeline::{Review, Step},
    },
    sessions::{LinkId, Session},
    settings::{PanelTab, SettingsStore},
    workspace::Workspace,
};

impl Workspace {
    pub(crate) fn open_task_result(
        &mut self,
        id: LinkId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.sessions.iter().any(|session| session.link_id == id) {
            return;
        }
        self.open_session_link(id, window, cx);
        self.quiet_companion(cx);
        self.butler.visible = true;
        self.expand_task(id, true, cx);
    }

    pub(super) fn expand_task(&mut self, id: LinkId, expanded: bool, cx: &mut Context<Self>) {
        self.butler.expanded = expanded.then_some(id);
        if expanded
            && let Some(session) = self
                .sessions
                .iter_mut()
                .find(|session| session.link_id == id)
            && let Some(entry) = session.log.completion()
        {
            let at = entry.at;
            if session.log.review(at, Review::Viewed) {
                log::info!("tasks: {} result viewed, not accepted", id.url());
                self.save_task_review(cx);
            }
        }
        cx.notify();
    }

    fn save_task_review(&mut self, cx: &mut Context<Self>) {
        if SettingsStore::get(cx).restore_sessions
            && let Err(error) = self.steps.flush(&self.sessions, Instant::now())
        {
            log::error!("tasks: review not saved: {error:#}");
            self.butler.error = Some(error.to_string());
            self.butler.target = self.butler.expanded;
        }
    }

    fn review_task(&mut self, id: LinkId, at: Instant, review: Review, cx: &mut Context<Self>) {
        let Some(session) = self
            .sessions
            .iter_mut()
            .find(|session| session.link_id == id)
        else {
            return;
        };
        if !matches!(
            session.task_state(cx),
            crate::sessions::task::TaskState::Unseen
                | crate::sessions::task::TaskState::Review
                | crate::sessions::task::TaskState::Accepted
        ) {
            return;
        }
        if session.log.review(at, review) {
            log::info!("tasks: {} result marked {review:?}", id.url());
            self.save_task_review(cx);
            cx.notify();
        }
    }

    fn result_panel(
        &mut self,
        id: LinkId,
        panel: PanelTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.sessions.iter().any(|session| session.link_id == id) {
            return;
        }
        match SettingsStore::update(cx, |settings| {
            settings.show_panel = true;
            settings.panel = panel;
        }) {
            Ok(()) => {
                self.open_session_link(id, window, cx);
                if panel == PanelTab::Activity
                    && let Some(session) = self.sessions.get_mut(self.active)
                    && let Some(round) = session.log.rounds().last()
                {
                    let at = round.at;
                    session.log.flipped.remove(&at);
                    if let Some(drawer) = session.drawer.as_mut() {
                        drawer.open = false;
                    }
                }
                self.dismiss_butler(cx);
                log::info!("tasks: {} opens {panel:?}", id.url());
            }
            Err(error) => {
                log::error!("tasks: result panel failed: {error:#}");
                self.butler.error = Some(error.to_string());
                self.butler.target = Some(id);
                cx.notify();
            }
        }
    }

    pub(super) fn task_result(&self, session: &Session, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let id = session.link_id;
        let word = |key| text(cx, key, &[]);
        let mut body = div().ml(rems(2.5)).flex().flex_col().gap_2();
        let Some(entry) = session.log.completion() else {
            return body;
        };
        let Step::Ended { failed, said, .. } = &entry.step else {
            return body;
        };
        let at = entry.at;
        body = body
            .child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .child(word("et.task.reply")),
            )
            .child(
                div().text_sm().child(
                    said.clone()
                        .filter(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| word("et.task.no_reply").to_string()),
                ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.colors.fg_muted)
                    .child(word("et.task.evidence")),
            );
        let rounds = session.log.rounds();
        if let Some(round) = rounds.last() {
            let runs: Vec<_> = round
                .steps
                .iter()
                .filter_map(|entry| match &entry.step {
                    Step::Tool {
                        doing: Doing::Running,
                        name,
                        detail,
                        took,
                        failed,
                        ..
                    } => {
                        let status = word(if took.is_none() {
                            "et.task.run_unknown"
                        } else if *failed {
                            "et.task.run_failed"
                        } else {
                            "et.task.run_returned"
                        });
                        Some(format!("{status} · {}", detail.as_deref().unwrap_or(name)))
                    }
                    _ => None,
                })
                .collect();
            body = body.child(
                div()
                    .font_weight(FontWeight::MEDIUM)
                    .child(word("et.task.runs")),
            );
            if runs.is_empty() {
                body = body.child(
                    div()
                        .text_xs()
                        .text_color(theme.colors.fg_muted)
                        .child(word("et.task.no_runs")),
                );
            } else {
                body = body.children(runs.into_iter().map(|run| div().text_xs().child(run)));
            }
        }
        body = body.child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .child(
                    Button::new(("result-git", session.id), word("et.task.changes"))
                        .size(ControlSize::Sm)
                        .variant(ButtonVariant::Ghost)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.result_panel(id, PanelTab::Git, window, cx)
                        })),
                )
                .child(
                    Button::new(("result-activity", session.id), word("et.task.activity"))
                        .size(ControlSize::Sm)
                        .variant(ButtonVariant::Ghost)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.result_panel(id, PanelTab::Activity, window, cx)
                        })),
                ),
        );
        body = body.child(
            div()
                .text_xs()
                .text_color(theme.colors.fg_muted)
                .child(word("et.task.shared_changes")),
        );
        if !failed
            && matches!(
                session.task_state(cx),
                crate::sessions::task::TaskState::Unseen
                    | crate::sessions::task::TaskState::Review
                    | crate::sessions::task::TaskState::Accepted
            )
        {
            let accepted = entry.review == Review::Accepted;
            body = body.child(
                Button::new(
                    ("result-review", session.id),
                    word(if accepted {
                        "et.task.reopen"
                    } else {
                        "et.task.accept"
                    }),
                )
                .size(ControlSize::Sm)
                .variant(ButtonVariant::Ghost)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.review_task(
                        id,
                        at,
                        if accepted {
                            Review::Unseen
                        } else {
                            Review::Accepted
                        },
                        cx,
                    )
                })),
            );
        }
        body
    }
}
