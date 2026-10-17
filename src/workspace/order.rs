//! The sessions' order, which their rows' drags change.

use gpui::Context;

use super::Workspace;
use crate::sessions;

impl Workspace {
    /// Moves session `from` to place `to`; the shown session stays shown.
    pub fn move_session(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let count = self.sessions.len();
        let to = to.min(count.saturating_sub(1));
        if from >= count || from == to {
            return;
        }
        let session = self.sessions.remove(from);
        log::info!(
            "app: session {} moves from place {} to {}",
            session.id,
            from + 1,
            to + 1
        );
        self.sessions.insert(to, session);
        self.active = sessions::after_move(self.active, from, to);
        cx.notify();
    }
}
