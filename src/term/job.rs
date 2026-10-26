//! What following a session's agent reads of its terminal, and the line a restored session types.

use std::time::Instant;

use super::view::TerminalView;

impl TerminalView {
    /// Types `line` and Enter, as the user would.
    pub fn type_line(&self, line: &str) {
        self.pty.write(format!("{line}\r").into_bytes());
    }

    /// The process group the program's keys go to.
    #[cfg(unix)]
    pub fn foreground(&self) -> Option<u32> {
        let group = self.pty.foreground().ok()??;
        u32::try_from(group).ok()
    }

    #[cfg(windows)]
    pub fn foreground(&self) -> Option<u32> {
        None
    }

    /// When the program last drew.
    pub fn drew(&self) -> Instant {
        self.drew
    }
}
