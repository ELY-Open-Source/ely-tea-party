//! An agent's prompts in the scrollback: the row each one shows on carries a mark, as a prompt's link, so it scrolls and trims with its text.

use std::time::{Duration, Instant};

use alacritty_terminal::{
    grid::{Dimensions, Scroll},
    index::{Column, Line},
    term::{
        Term,
        cell::{Flags, Hyperlink},
    },
};
use gpui::Context;

use super::{prompts::LINK, view::TerminalView};

/// What a mark's link id starts with; a prompt's own carries a bare number.
const MARK: &str = "\u{7f}t";

/// The most prompts a terminal keeps; older ones drop.
const KEPT: usize = 200;

/// How many characters of a prompt find its row.
const PROBE: usize = 16;

/// How many prompts never found are still sought.
const SOUGHT: usize = 3;

/// How often rows are read for a prompt not yet found.
const SEEK_EVERY: Duration = Duration::from_millis(250);

/// Where a prompt is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// Not yet found, or lost from the row the agent redrew.
    Seeking,
    /// On this line: negative in the scrollback.
    At(i32),
    /// Trimmed from the scrollback.
    Gone,
    /// Never found.
    Lost,
}

/// A prompt and how its turn ended.
#[derive(Clone, Debug)]
pub struct Turn {
    id: u32,
    pub prompt: String,
    pub reply: Option<String>,
    pub place: Place,
    marked: bool,
}

/// The prompts an agent took in this terminal, in order.
#[derive(Default)]
pub struct Turns {
    pub list: Vec<Turn>,
    next: u32,
    sought: Option<Instant>,
}

impl Turns {
    pub fn prompted(&mut self, prompt: String) {
        self.next += 1;
        self.list.push(Turn {
            id: self.next,
            prompt,
            reply: None,
            place: Place::Seeking,
            marked: false,
        });
        if self.list.len() > KEPT {
            self.list.remove(0);
        }
        self.sought = None;
    }

    /// The last turn ended saying `reply`.
    pub fn replied(&mut self, reply: String) {
        if let Some(turn) = self.list.last_mut() {
            turn.reply = Some(reply);
        }
    }

    /// Reads where each prompt's mark is now, and seeks and marks the rows of those not found.
    pub fn follow<T>(&mut self, term: &mut Term<T>, now: Instant) {
        if self.list.is_empty() {
            return;
        }
        let (top, bottom) = (term.topmost_line().0, term.bottommost_line().0);
        let mut found = Vec::new();
        for line in top..=bottom {
            let cell = &term.grid()[Line(line)][Column(0)];
            if let Some(id) = cell.hyperlink().and_then(|link| mark(link.id())) {
                found.push((id, line));
            }
        }
        let last = self.list.len() - 1;
        for (index, turn) in self.list.iter_mut().enumerate() {
            turn.place = match found.iter().find(|(id, _)| *id == turn.id) {
                Some(&(_, line)) => Place::At(line),
                None if turn.marked && index < last => Place::Gone,
                None if turn.marked || last - index < SOUGHT => Place::Seeking,
                None => Place::Lost,
            };
            turn.marked |= matches!(turn.place, Place::At(_));
        }
        let due = self
            .sought
            .is_none_or(|at| now.saturating_duration_since(at) >= SEEK_EVERY);
        if !due || !self.list.iter().any(|turn| turn.place == Place::Seeking) {
            return;
        }
        self.sought = Some(now);
        for index in 0..self.list.len() {
            if self.list[index].place != Place::Seeking {
                continue;
            }
            let at = |turn: &Turn| match turn.place {
                Place::At(line) => Some(line),
                _ => None,
            };
            let from = self.list[..index]
                .iter()
                .rev()
                .find_map(at)
                .map_or(top, |line| line + 1);
            let to = self.list[index + 1..]
                .iter()
                .find_map(at)
                .map_or(bottom, |line| line - 1);
            let Some(line) = seek(term, &self.list[index].prompt, from, to) else {
                continue;
            };
            let turn = &mut self.list[index];
            let link = Hyperlink::new(Some(format!("{MARK}{}", turn.id)), LINK.to_string());
            term.grid_mut()[Line(line)][Column(0)].set_hyperlink(Some(link));
            turn.place = Place::At(line);
            turn.marked = true;
            log::debug!("turns: prompt {} found on line {line}", turn.id);
        }
    }
}

fn mark(id: &str) -> Option<u32> {
    id.strip_prefix(MARK)?.parse().ok()
}

/// The start of `prompt`'s first line that finds its row: past images' and pastes' placeholders, spaces collapsed.
fn probe(prompt: &str) -> String {
    let mut line = prompt
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    while let Some(rest) = line.strip_prefix('[').and_then(|rest| rest.split_once(']')) {
        line = rest.1.trim_start();
    }
    let words: Vec<&str> = line.split_whitespace().collect();
    words.join(" ").chars().take(PROBE).collect()
}

/// The text `line` shows, spaces collapsed.
fn text<T>(term: &Term<T>, line: i32) -> String {
    let row = &term.grid()[Line(line)];
    let spacers = Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER;
    let shown: String = (0..term.columns())
        .map(|column| &row[Column(column)])
        .filter(|cell| !cell.flags.intersects(spacers))
        .map(|cell| cell.c)
        .collect();
    shown.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The first line from `from` to `to` that shows `prompt`.
fn seek<T>(term: &Term<T>, prompt: &str, from: i32, to: i32) -> Option<i32> {
    let probe = probe(prompt);
    if probe.is_empty() {
        return None;
    }
    (from..=to).find(|&line| text(term, line).contains(&probe))
}

impl TerminalView {
    /// An agent in this terminal took `prompt`.
    pub fn prompted(&mut self, prompt: String, cx: &mut Context<Self>) {
        log::debug!("terminal {}: an agent took a prompt", self.pty.pid);
        self.turns.prompted(prompt);
        cx.notify();
    }

    /// The agent's turn ended saying `reply`.
    pub fn replied(&mut self, reply: String, cx: &mut Context<Self>) {
        self.turns.replied(reply);
        cx.notify();
    }

    /// Scrolls so prompt `line` shows a row below the view's top.
    pub(super) fn scroll_to_turn(&mut self, line: i32, cx: &mut Context<Self>) {
        let mut term = self.pty.term.lock();
        let history = term.grid().history_size() as i32;
        let offset = (1 - line).clamp(0, history);
        let delta = offset - term.grid().display_offset() as i32;
        term.scroll_display(Scroll::Delta(delta));
        drop(term);
        log::debug!("terminal {}: to the prompt on line {line}", self.pty.pid);
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
