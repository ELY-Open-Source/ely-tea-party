use std::sync::Arc;

use alacritty_terminal::{
    grid::Dimensions,
    index::{Column, Line},
    sync::FairMutex,
    term::{Term, TermMode, cell::Flags},
};
use serde::Serialize;

use super::{Listener, TerminalView};

pub struct SnapshotSource {
    pub id: String,
    pub url: String,
    pub title: String,
    pub folder: String,
    term: Arc<FairMutex<Term<Listener>>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: String,
    pub url: String,
    pub title: String,
    pub folder: String,
    pub captured_at: String,
    pub alternate_screen: bool,
    pub truncated: bool,
    pub lines: Vec<String>,
}

impl TerminalView {
    pub fn snapshot_source(
        &self,
        id: String,
        title: String,
        folder: String,
        url: String,
    ) -> SnapshotSource {
        SnapshotSource {
            id,
            url,
            title,
            folder,
            term: self.pty.term.clone(),
        }
    }
}

impl SnapshotSource {
    pub fn capture(self) -> Snapshot {
        let captured_at = chrono::Utc::now().to_rfc3339();
        let (lines, truncated, alternate_screen) = {
            let term = self.term.lock();
            let (lines, truncated) = tail(&term);
            (lines, truncated, term.mode().contains(TermMode::ALT_SCREEN))
        };
        Snapshot {
            id: self.id,
            url: self.url,
            title: self.title,
            folder: self.folder,
            captured_at,
            alternate_screen,
            truncated,
            lines,
        }
    }
}

fn tail<T>(term: &Term<T>) -> (Vec<String>, bool) {
    let top = term.topmost_line().0;
    let bottom = term.bottommost_line().0;
    let start = top.max(bottom - 399);
    let mut truncated = start > top;
    let mut bytes = 0;
    let mut rows = Vec::new();
    for line in (start..=bottom).rev() {
        let row = &term.grid()[Line(line)];
        let mut text = String::new();
        for column in 0..term.columns().min(2048) {
            let cell = &row[Column(column)];
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            if cell.flags.contains(Flags::HIDDEN) {
                text.push(' ');
            } else {
                text.push(if cell.c.is_control() { ' ' } else { cell.c });
                text.extend(cell.zerowidth().into_iter().flatten().copied().take(8));
            }
        }
        let wraps = row[term.last_column()].flags.contains(Flags::WRAPLINE);
        let text = if wraps {
            text
        } else {
            text.trim_end().to_string()
        };
        bytes += text.len();
        if bytes > 32_000 {
            truncated = true;
            break;
        }
        truncated |= term.columns() > 2048;
        rows.push((
            text,
            row[term.last_column()].flags.contains(Flags::WRAPLINE),
        ));
    }
    rows.reverse();
    let mut lines = Vec::new();
    let mut logical = String::new();
    for (text, wraps) in rows {
        logical.push_str(&text);
        if !wraps {
            lines.push(std::mem::take(&mut logical));
        }
    }
    if !logical.is_empty() {
        lines.push(logical);
    }
    (lines, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alacritty_terminal::{event::VoidListener, term::Config, vte::ansi};

    struct Size;
    impl Dimensions for Size {
        fn total_lines(&self) -> usize {
            4
        }
        fn screen_lines(&self) -> usize {
            4
        }
        fn columns(&self) -> usize {
            12
        }
    }

    #[test]
    fn reads_scrollback_independent_of_viewport_and_hides_hidden_cells() {
        let mut term = Term::new(Config::default(), &Size, VoidListener);
        let mut parser: ansi::Processor = ansi::Processor::new();
        parser.advance(
            &mut term,
            b"first\r\nsecond\r\nthird\r\nfourth\r\nfifth\r\n\x1b[8msecret\x1b[0m",
        );
        let before = tail(&term);
        term.scroll_display(alacritty_terminal::grid::Scroll::Top);
        assert_eq!(before, tail(&term));
        assert!(before.0.join("\n").contains("first"));
        assert!(!before.0.join("\n").contains("secret"));
    }

    #[test]
    fn joins_soft_wraps_and_bounds_history() {
        let mut term = Term::new(Config::default(), &Size, VoidListener);
        let mut parser: ansi::Processor = ansi::Processor::new();
        parser.advance(&mut term, b"abcdefghijklmnop");
        assert!(tail(&term).0.join("\n").contains("abcdefghijklmnop"));
        for _ in 0..500 {
            parser.advance(&mut term, b"\r\nline");
        }
        let (lines, truncated) = tail(&term);
        assert!(truncated);
        assert!(lines.len() <= 400);
    }
}
