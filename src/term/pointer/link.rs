//! The pointer over a link while the open modifier is held.

use gpui::{Context, Modifiers, ModifiersChangedEvent, Window};

use super::{TerminalView, opens};

impl TerminalView {
    /// Points the pointer when the open modifier is down over a link.
    pub(super) fn hover(&mut self, modifiers: &Modifiers, cx: &mut Context<Self>) {
        let on_link = opens(modifiers) && {
            let (_, point, _) = self.cell_at(self.pointer.at);
            self.link(point).is_some()
        };
        if self.pointer.on_link != on_link {
            self.pointer.on_link = on_link;
            cx.notify();
        }
    }

    pub(in crate::term) fn modifiers_changed(
        &mut self,
        event: &ModifiersChangedEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.hover(&event.modifiers, cx);
    }
}
