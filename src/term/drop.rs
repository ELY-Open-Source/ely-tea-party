//! Files dropped on the terminal type their paths, as macOS Terminal does.

use std::path::PathBuf;

use anyhow::Context as _;
use gpui::{Context, ExternalPaths};

use super::view::TerminalView;

/// What a POSIX shell reads specially in a path: Ghostty's list, and zsh's `extendedglob` marks.
const SPECIAL: &str = "\\ ()[]{}<>\"'`!#$&;|*?^~";

/// PowerShell's single quotes, each one doubled inside a quoted string.
const SINGLE: [char; 5] = ['\'', '\u{2018}', '\u{2019}', '\u{201a}', '\u{201b}'];

/// How a session's shell reads a path typed into it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quote {
    /// sh, bash, zsh, fish: a backslash before each special character.
    Posix,
    /// Single quotes, which expand nothing.
    PowerShell,
    /// Double quotes, past which cmd still expands `%` and, under `/v:on`, `!`.
    Cmd,
}

/// The text `paths` type, each followed by a space, as `quote` keeps each one literal.
pub fn typed(paths: &[PathBuf], quote: Quote) -> anyhow::Result<String> {
    paths
        .iter()
        .map(|path| {
            let text = (path.to_str())
                .with_context(|| format!("{} is no UTF-8 a session can type", path.display()))?;
            anyhow::ensure!(
                !text.chars().any(char::is_control),
                "{text:?} holds a control character, which would act as typed"
            );
            let mut out = String::with_capacity(text.len() + 3);
            match quote {
                Quote::Posix => text.chars().for_each(|ch| {
                    if SPECIAL.contains(ch) {
                        out.push('\\');
                    }
                    out.push(ch);
                }),
                Quote::PowerShell => {
                    out.push('\'');
                    text.chars().for_each(|ch| {
                        if SINGLE.contains(&ch) {
                            out.push(ch);
                        }
                        out.push(ch);
                    });
                    out.push('\'');
                }
                Quote::Cmd => {
                    anyhow::ensure!(
                        !text.contains(['%', '!', '"']),
                        "{text:?} holds %, ! or \", which cmd reads inside quotes"
                    );
                    // A program reads a backslash before the closing quote as a quote.
                    let trailing = text.len() - text.trim_end_matches('\\').len();
                    out = format!("\"{text}{}\"", "\\".repeat(trailing));
                }
            }
            out.push(' ');
            Ok(out)
        })
        .collect()
}

impl TerminalView {
    /// Types the dropped paths at the cursor as a paste; a path it cannot type stops the drop.
    pub(super) fn drop_paths(&mut self, dropped: &ExternalPaths, cx: &mut Context<Self>) {
        let (paths, quote) = (dropped.paths(), self.pty.quote);
        match typed(paths, quote) {
            Ok(text) => {
                log::info!(
                    "terminal {}: {} paths dropped, quoted for {quote:?}",
                    self.pty.pid,
                    paths.len()
                );
                self.paste_text(&text, cx);
            }
            Err(error) => log::error!(
                "terminal {}: the drop types nothing: {error:#}",
                self.pty.pid
            ),
        }
    }
}

#[cfg(test)]
mod tests;
