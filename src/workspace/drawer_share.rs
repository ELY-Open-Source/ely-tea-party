mod redact;

use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    forms::{InputEvent, TextInput},
    i18n::text,
    overlays::Dialog,
    primitives::Tooltip,
    theme::ControlSize,
    typography::Caption,
};
use gpui::{
    AppContext, Context, Entity, EntityId, Focusable, InteractiveElement, IntoElement,
    ParentElement, StatefulInteractiveElement, Styled, Subscription, Window, div,
};

use super::Workspace;
use crate::agents::detect::{Foreground, foreground};

pub(super) struct Preview {
    session: u64,
    terminal: EntityId,
    agent: uuid::Uuid,
    foreground: Option<u32>,
    input: Entity<TextInput>,
    truncated: bool,
    error: Option<&'static str>,
    _changes: Subscription,
}

impl Workspace {
    pub(super) fn drawer_share_button(&self, cx: &Context<Self>) -> impl IntoElement {
        let enabled = self
            .sessions
            .get(self.active)
            .is_some_and(|s| s.agent.is_some());
        let workspace = cx.entity();
        div()
            .id("drawer-share-footer")
            .tooltip(Tooltip::text(text(cx, "et.drawer.share_hint", &[])))
            .flex()
            .flex_none()
            .justify_end()
            .px_3()
            .py_1()
            .child(
                Button::new("share-drawer", text(cx, "et.drawer.share", &[]))
                    .variant(ButtonVariant::Secondary)
                    .size(ControlSize::Sm)
                    .disabled(!enabled)
                    .on_click(move |_, window, cx| {
                        workspace.update(cx, |this, cx| this.preview_drawer(window, cx));
                    }),
            )
    }

    fn preview_drawer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.sessions.get(self.active) else {
            return;
        };
        let Some(drawer) = &session.drawer else {
            return;
        };
        let Some(agent) = &session.agent else { return };
        let session_id = session.id;
        let terminal = session.terminal.entity_id();
        let group = session.terminal.read(cx).foreground();
        let agent = agent.instance;
        let snapshot = drawer
            .terminal
            .read(cx)
            .snapshot_source(String::new(), String::new(), String::new(), String::new())
            .capture();
        let mut lines = snapshot.lines;
        // A bounded capture may begin halfway through a secret.
        if snapshot.truncated && !lines.is_empty() {
            lines.remove(0);
        }
        let sanitized = redact::redact(&lines.join("\n"));
        let input = cx.new(|cx| {
            let mut input = TextInput::new(window, cx).multi_line(8, 16).max_len(40_000);
            input.set_text(sanitized, cx);
            input
        });
        let changes = cx.subscribe(&input, |_, _, event, cx| {
            if matches!(event, InputEvent::Changed) {
                cx.notify();
            }
        });
        self.drawer_share = Some(Preview {
            session: session_id,
            terminal,
            agent,
            foreground: group,
            input,
            truncated: snapshot.truncated,
            error: None,
            _changes: changes,
        });
        log::info!("drawer: session {session_id} opens a redacted preview");
        cx.notify();
    }

    fn insert_drawer(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<Entity<crate::term::TerminalView>> {
        let preview = self.drawer_share.as_ref()?;
        let message = preview.input.read(cx).text().to_string();
        let sanitized = redact::redact(&message);
        if sanitized != message {
            preview
                .input
                .update(cx, |input, cx| input.set_text(sanitized, cx));
            self.drawer_share.as_mut()?.error = Some("et.drawer.redacted_again");
            cx.notify();
            return None;
        }
        if message.trim().is_empty() {
            return None;
        }
        let target = self
            .sessions
            .get(self.active)
            .filter(|session| {
                session.id == preview.session
                    && session.terminal.entity_id() == preview.terminal
                    && session
                        .agent
                        .as_ref()
                        .is_some_and(|agent| agent.instance == preview.agent)
            })
            .map(|session| session.terminal.clone());
        let target = target.filter(|terminal| {
            let view = terminal.read(cx);
            view.foreground() == preview.foreground
                && foreground(view.pid(), view.foreground()) != Foreground::Shell
        });
        let Some(target) = target else {
            log::warn!("drawer: preview destination changed; nothing inserted");
            self.drawer_share.as_mut()?.error = Some("et.drawer.target_changed");
            cx.notify();
            return None;
        };
        if !target.update(cx, |view, cx| view.paste_context(&message, cx)) {
            self.drawer_share.as_mut()?.error = Some("et.drawer.no_paste");
            cx.notify();
            return None;
        }
        log::info!(
            "drawer: redacted context inserted into session {}",
            preview.session
        );
        Some(target)
    }

    pub(super) fn drawer_share_dialog(&self, cx: &Context<Self>) -> Option<Dialog> {
        let preview = self.drawer_share.as_ref()?;
        let owner = cx.entity();
        let send = owner.clone();
        let cancel = text(cx, "et.drawer.cancel", &[]);
        let insert = text(cx, "et.drawer.insert", &[]);
        let empty = preview.input.read(cx).text().trim().is_empty();
        Some(
            Dialog::new(
                "drawer-share-preview",
                text(cx, "et.drawer.preview", &[]),
                move |_, cx| {
                    owner.update(cx, |this, cx| {
                        this.drawer_share = None;
                        log::debug!("drawer: preview closed");
                        cx.notify();
                    });
                },
            )
            .detail(text(cx, "et.drawer.privacy", &[]))
            .child(Caption::new(text(cx, "et.drawer.scope", &[])))
            .children(
                preview
                    .truncated
                    .then(|| Caption::new(text(cx, "et.drawer.truncated", &[]))),
            )
            .child(preview.input.clone())
            .children(preview.error.map(|key| Caption::new(text(cx, key, &[]))))
            .action(move |close| {
                Button::new("cancel-drawer-share", cancel)
                    .variant(ButtonVariant::Secondary)
                    .on_click(move |_, window, cx| close(window, cx))
            })
            .action(move |close| {
                Button::new("insert-drawer-share", insert)
                    .variant(ButtonVariant::Primary)
                    .disabled(empty)
                    .on_click(move |_, window, cx| {
                        if let Some(target) = send.update(cx, |this, cx| this.insert_drawer(cx)) {
                            close(window, cx);
                            window.focus(&target.focus_handle(cx), cx);
                        }
                    })
            }),
        )
    }
}
