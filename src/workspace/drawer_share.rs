mod redact;

use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    i18n::text,
    primitives::Tooltip,
    theme::ControlSize,
    typography::Caption,
};
use gpui::{
    Context, Entity, EntityId, Focusable, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div,
};

use super::{Workspace, drawer::Drawer};
use crate::{
    agents::detect::{Foreground, foreground},
    term::TerminalView,
};

impl Workspace {
    pub(super) fn drawer_share_button(
        &self,
        drawer: &Drawer,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let enabled = self
            .sessions
            .get(self.active)
            .is_some_and(|session| session.agent.is_some());
        let source = drawer.terminal.entity_id();
        div()
            .id("drawer-share-footer")
            .tooltip(Tooltip::text(text(cx, "et.drawer.share_hint", &[])))
            .flex()
            .flex_none()
            .items_center()
            .justify_end()
            .gap_2()
            .px_3()
            .py_1()
            .children(
                drawer
                    .share_error
                    .map(|key| Caption::new(text(cx, key, &[]))),
            )
            .child(
                div().flex_none().child(
                    Button::new("share-drawer", text(cx, "et.drawer.share", &[]))
                        .variant(ButtonVariant::Secondary)
                        .size(ControlSize::Sm)
                        .disabled(!enabled)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.send_drawer(source, window, cx);
                        })),
                ),
            )
    }

    fn send_drawer(&mut self, source: EntityId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = self.sessions.get_mut(self.active) else {
            return;
        };
        let target = session.main().terminal.clone();
        let Some(drawer) = session
            .drawer
            .as_mut()
            .filter(|drawer| drawer.open && drawer.terminal.entity_id() == source)
        else {
            return;
        };
        let result = if session.agent.is_some() {
            paste_drawer(drawer, &target, cx)
        } else {
            Err("et.drawer.target_changed")
        };
        drawer.share_error = result.err();
        if let Some(error) = drawer.share_error {
            log::warn!(
                "drawer: context for session {} skipped ({error})",
                session.id
            );
        } else {
            log::info!(
                "drawer: redacted context inserted into session {}",
                session.id
            );
            window.focus(&target.focus_handle(cx), cx);
        }
        cx.notify();
    }
}

fn paste_drawer(
    drawer: &Drawer,
    target: &Entity<TerminalView>,
    cx: &mut Context<Workspace>,
) -> Result<(), &'static str> {
    let view = target.read(cx);
    let group = view.foreground();
    if foreground(view.pid(), group) == Foreground::Shell {
        return Err("et.drawer.target_changed");
    }
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
    let message = redact::redact(&lines.join("\n"));
    if message.trim().is_empty() {
        return Err("et.drawer.empty");
    }
    target.update(cx, |view, cx| {
        if view.foreground() != group {
            return Err("et.drawer.target_changed");
        }
        if !view.paste_context(&message, cx) {
            return Err("et.drawer.no_paste");
        }
        Ok(())
    })
}
