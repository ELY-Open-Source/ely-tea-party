use alacritty_terminal::{
    grid::{Dimensions, Row, Scroll},
    index::Line,
    term::{Term, cell::Cell},
};
use gpui::Context;

use super::{tap::MOST, view::TerminalView};

/// The target of the OSC 8 link a prompt becomes; no program names a link so.
pub(crate) const LINK: &str = "\u{7f}";

/// What a link's id starts with when it carries a prompt's number: the prompt's own link, or a program's inside it.
const NUMBERED: char = '\u{7f}';

/// What ends a prompt's link.
const CLOSE: &[u8] = b"\x1b]8;;\x07";

/// What a held sequence's rewrite comes to at most: a link with the longest target the scanner keeps.
pub(super) const SLACK: usize = MOST + 32;

/// How many times its size a rewrite makes a sequence at most: a mark of 8 bytes inside a program's link becomes that link again.
pub(super) const GROWTH: usize = SLACK / 8 + 1;

const ESC: u8 = 0x1b;

/// Controls vte skips inside an OSC.
fn ignored(byte: u8) -> bool {
    matches!(byte, 0x00..=0x06 | 0x08..=0x17 | 0x19 | 0x1c..=0x1f)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum State {
    #[default]
    Ground,
    /// After an ESC, which waits while the parser stands where it stood before it, else has gone on.
    Escape { sent: bool },
    /// An OSC held whole, since it may be a mark or a link to rewrite.
    Held,
    /// An OSC passed on as it comes.
    Passing,
}

/// Rewrites what goes to the parser so it stamps each prompt's cells with a link of its own, numbered per prompt.
#[derive(Default)]
pub(crate) struct Prompts {
    state: State,
    /// The escape or OSC held, from its ESC.
    held: Vec<u8>,
    /// The OSC's body as vte reads it, the controls it skips left out.
    body: Vec<u8>,
    /// The body passed the scanner's bound, so it counts for nothing.
    overflow: bool,
    /// The held OSC's ESC has gone on: it ended the OSC before.
    sent: bool,
    /// The prompt open now, from its start mark to its end.
    prompt: Option<u32>,
    /// The target of a program's link open now, to open it again with a prompt's number and without.
    target: Option<Vec<u8>>,
    /// The last prompt's number.
    count: u32,
    /// What goes to the parser from the last read.
    out: Vec<u8>,
}

impl Prompts {
    /// The bytes held from earlier reads.
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// What goes to the parser for `read`: marks become links, and a held start waits for its end.
    pub fn pass(&mut self, read: &[u8]) -> &[u8] {
        self.out.clear();
        for &byte in read {
            self.step(byte);
        }
        &self.out
    }

    fn step(&mut self, byte: u8) {
        self.state = match (self.state, byte) {
            (State::Ground, ESC) => {
                self.held.push(ESC);
                State::Escape { sent: false }
            }
            (State::Ground, _) => {
                self.out.push(byte);
                State::Ground
            }
            (State::Escape { sent }, b']') => {
                self.held.push(byte);
                self.sent = sent;
                self.body.clear();
                self.overflow = false;
                State::Held
            }
            (State::Escape { .. }, ESC) => {
                self.release();
                self.held.push(ESC);
                State::Escape { sent: false }
            }
            // vte runs these and stays in the escape; one that waits lets them on first, where the parser stands the same.
            (State::Escape { sent }, _) if byte == 0x07 || ignored(byte) => {
                self.out.push(byte);
                State::Escape { sent }
            }
            // vte passes DEL and high bytes by in the escape: the ESC goes on, then they.
            (State::Escape { sent }, 0x7f..=0xff) => {
                if !sent {
                    self.release();
                }
                self.out.push(byte);
                State::Escape { sent: true }
            }
            (State::Escape { .. }, _) => {
                // A full reset clears the link the parser writes with.
                if byte == b'c' {
                    (self.prompt, self.target) = (None, None);
                }
                self.held.push(byte);
                self.release();
                State::Ground
            }
            (State::Held | State::Passing, 0x07 | 0x18 | 0x1a) => {
                self.end(Some(byte));
                State::Ground
            }
            // An ESC ends the OSC: after a rewrite the parser stands in ground, else inside the OSC, so the ESC goes on.
            (State::Held | State::Passing, ESC) => {
                if self.end(None) {
                    self.held.push(ESC);
                    State::Escape { sent: false }
                } else {
                    self.out.push(ESC);
                    State::Escape { sent: true }
                }
            }
            (State::Held, _) => {
                self.held.push(byte);
                self.read(byte);
                if self.wanted() {
                    State::Held
                } else {
                    self.release();
                    State::Passing
                }
            }
            (State::Passing, _) => {
                self.out.push(byte);
                self.read(byte);
                State::Passing
            }
        };
    }

    /// Adds `byte` to the body as the tap's scanner does; skipped controls bound the bytes held too.
    fn read(&mut self, byte: u8) {
        if self.held.len() > 2 * MOST {
            self.overflow = true;
        }
        if ignored(byte) {
            return;
        }
        if self.body.len() < MOST {
            self.body.push(byte);
        } else {
            self.overflow = true;
        }
    }

    /// Whether the OSC held may be a mark, or a link that ends inside a prompt, within the scanner's bounds.
    fn wanted(&self) -> bool {
        let fits = |prefix: &[u8]| {
            let length = self.body.len().min(prefix.len());
            self.body[..length] == prefix[..length]
        };
        !self.overflow && (fits(b"133;") || (self.prompt.is_some() && fits(b"8;")))
    }

    fn release(&mut self) {
        self.out.append(&mut self.held);
    }

    /// The OSC ends at `end`, BEL, CAN or SUB, or at an ESC when none; true when it was rewritten.
    fn end(&mut self, end: Option<u8>) -> bool {
        let rewrite = if self.overflow {
            None
        } else if let Some(opens) = self.body.strip_prefix(b"133;").and_then(mark) {
            if opens {
                self.count = self.count.wrapping_add(1);
                self.prompt = Some(self.count);
                // A prompt inside a program's link carries its number on that link.
                Some(match &self.target {
                    Some(target) => link(self.count, target),
                    None => link(self.count, LINK.as_bytes()),
                })
            } else {
                match (self.prompt.take(), &self.target) {
                    (Some(_), Some(target)) => Some(plain(target)),
                    (Some(_), None) => Some(CLOSE.to_vec()),
                    (None, _) => None,
                }
            }
        } else if let Some((opens, target)) = user(&self.body) {
            self.target = opens.then(|| target.to_vec());
            // Inside a prompt a program's link carries the prompt's number; its end takes the prompt's link up again.
            let target = if opens { target } else { LINK.as_bytes() };
            self.prompt.map(|number| link(number, target))
        } else {
            None
        };
        match (rewrite, self.state) {
            (Some(bytes), State::Held) => {
                self.held.clear();
                // An ESC that went on already starts the link.
                self.out.extend_from_slice(&bytes[usize::from(self.sent)..]);
                // CAN and SUB end the OSC and run as well.
                self.out.extend(end.filter(|&byte| byte != 0x07));
                true
            }
            (_, State::Held) => {
                self.held.extend(end);
                self.release();
                false
            }
            _ => {
                self.out.extend(end);
                false
            }
        }
    }
}

/// Whether a mark's body opens a prompt or ends one, as `marks::read` reads its letter; none for neither.
fn mark(body: &[u8]) -> Option<bool> {
    let (&letter, rest) = body.split_first()?;
    if !rest.is_empty() && rest[0] != b';' {
        return None;
    }
    match letter {
        b'A' | b'N' => Some(true),
        b'B' | b'I' | b'C' | b'D' => Some(false),
        _ => None,
    }
}

/// Whether an OSC 8 body opens a program's link, as alacritty reads its target, and that target; none for no link at all.
fn user(body: &[u8]) -> Option<(bool, &[u8])> {
    let rest = body.strip_prefix(b"8;")?;
    let target = &rest[rest.iter().position(|&byte| byte == b';')? + 1..];
    // alacritty joins the target's parts with `;`, each one empty when it is not UTF-8.
    let opens =
        target.contains(&b';') || std::str::from_utf8(target).is_ok_and(|text| !text.is_empty());
    Some((opens, target))
}

/// A link to `target` whose id carries prompt `number`.
fn link(number: u32, target: &[u8]) -> Vec<u8> {
    let mut link = format!("\x1b]8;id={NUMBERED}{number};").into_bytes();
    link.extend_from_slice(target);
    link.push(0x07);
    link
}

/// A program's link to `target`, its id its own again.
fn plain(target: &[u8]) -> Vec<u8> {
    let mut link = b"\x1b]8;;".to_vec();
    link.extend_from_slice(target);
    link.push(0x07);
    link
}

/// The number of the prompt `row` shows, as a link's id carries it.
fn prompt(row: &Row<Cell>) -> Option<u32> {
    row.into_iter().find_map(|cell| {
        let link = cell.hyperlink()?;
        link.id().strip_prefix(NUMBERED)?.parse().ok()
    })
}

/// The display offset that puts the prompt before or after the view's top at the top, as Ghostty jumps.
pub fn jump<T>(term: &Term<T>, up: bool) -> Option<usize> {
    let top = -(term.grid().display_offset() as i32);
    let (first, last) = (term.topmost_line().0, term.bottommost_line().0);
    let number = |line: i32| prompt(&term.grid()[Line(line)]);
    // ponytail: a scan of the rows on the way; index prompt rows if long histories lag.
    let found = if up {
        // The nearest prompt above the view's top, then its first row, blank rows within it passed.
        let (line, own) = (first..top)
            .rev()
            .find_map(|line| number(line).map(|own| (line, own)))?;
        let mut start = line;
        for above in (first..line).rev() {
            match number(above) {
                Some(other) if other != own => break,
                Some(_) => start = above,
                None => {}
            }
        }
        start
    } else {
        // The prompt at or above the view's top may go on below it; the next one starts.
        let here = (first..=top).rev().find_map(number);
        (top + 1..=last).find(|&line| number(line).is_some_and(|other| Some(other) != here))?
    };
    // A prompt on the screen means the bottom, which the view cannot pass.
    Some((-found).max(0) as usize)
}

impl TerminalView {
    /// Scrolls to the prompt before or after the view's top; with none there, nothing moves.
    pub(super) fn scroll_to_prompt(&mut self, up: bool, cx: &mut Context<Self>) {
        let mut term = self.pty.term.lock();
        let Some(offset) = jump(&term, up) else {
            log::debug!(
                "terminal {}: no prompt {}",
                self.pty.pid,
                if up { "above" } else { "below" }
            );
            return;
        };
        let delta = offset as i32 - term.grid().display_offset() as i32;
        term.scroll_display(Scroll::Delta(delta));
        drop(term);
        log::debug!("terminal {}: to the prompt {offset} lines up", self.pty.pid);
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
