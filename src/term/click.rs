//! A click on the line the shell reads moves its cursor there, as Ghostty's `cursor-click-to-move`: arrows, or the press itself where the shell takes clicks.

use alacritty_terminal::{
    grid::Dimensions,
    index::{Column, Line, Point},
    term::{
        Term, TermMode,
        cell::{Cell, Flags},
    },
};
use gpui::Context;

use super::{
    marks::Click,
    mouse::Tracking,
    prompts::{Part, part, prompted, start},
    view::TerminalView,
};

/// Where the shell stands, as its OSC 133 marks say.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shell {
    /// A command runs, or nothing said otherwise.
    #[default]
    Busy,
    /// Its prompt draws; the line after it takes a click so.
    Prompting(Click),
    /// It reads a line, which takes a click so.
    Reading(Click),
}

impl Shell {
    /// The shell after B: it reads the line its prompt started, which takes arrows without one.
    pub fn reads(self) -> Self {
        match self {
            Self::Prompting(click) | Self::Reading(click) => Self::Reading(click),
            Self::Busy => Self::Reading(Click::Arrows),
        }
    }
}

/// Arrows from the cursor to `click`, right positive; none when `click` lies off the input after the cursor's prompt, soft wraps joined.
pub fn arrows<T>(term: &Term<T>, click: Point) -> Option<i32> {
    if term.mode().contains(TermMode::ALT_SCREEN) {
        return None;
    }
    let grid = term.grid();
    let columns = grid.columns();
    let cursor = grid.cursor.point;
    let wraps = |line: i32| {
        grid[Line(line)][Column(columns - 1)]
            .flags
            .contains(Flags::WRAPLINE)
    };
    let mut first = cursor.line.0;
    while first > grid.topmost_line().0 && wraps(first - 1) {
        first -= 1;
    }
    let mut last = cursor.line.0;
    while last < grid.bottommost_line().0 && wraps(last) {
        last += 1;
    }
    if !(first..=last).contains(&click.line.0) {
        return None;
    }
    let cells: Vec<&Cell> = (first..=last)
        .flat_map(|line| (0..columns).map(move |column| &grid[Line(line)][Column(column)]))
        .collect();
    let at = |point: Point| (point.line.0 - first) as usize * columns + point.column.0;
    let (here, there) = (at(cursor), at(click));
    // Input starts past the prompt the cursor's line shows before it.
    let start = cells[..here].iter().rposition(|cell| prompted(cell))? + 1;
    if there < start {
        return None;
    }
    let inked = |cell: &&Cell| !prompted(cell) && !matches!(cell.c, ' ' | '\t');
    let end = cells[start..]
        .iter()
        .rposition(inked)
        .map_or(here, |last| here.max(start + last + 1));
    let mut target = there.min(end);
    if cells[target].flags.contains(Flags::WIDE_CHAR_SPACER) {
        target -= 1;
    }
    let spacer = Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER;
    let typed = |cell: &&&Cell| !prompted(cell) && !cell.flags.intersects(spacer);
    let count = |from: usize, to: usize| cells[from..to].iter().filter(typed).count() as i32;
    Some(if target >= here {
        count(here, target)
    } else {
        -count(target, here)
    })
}

/// The left press, SGR, a shell that takes clicks hears for `click`, its row the screen's or counted from its prompt's first, as kitty and Ghostty send it; none on a prompt, before the input or below it.
pub fn press<T>(term: &Term<T>, click: Point, relative: bool) -> Option<Vec<u8>> {
    if term.mode().contains(TermMode::ALT_SCREEN) {
        return None;
    }
    let grid = term.grid();
    let cell = |(line, column): (i32, usize)| &grid[Line(line)][Column(column)];
    let cursor = (grid.cursor.point.line.0, grid.cursor.point.column.0);
    // ponytail: scans of the rows on the way; index the terminal's own links if long histories lag.
    let back = (grid.topmost_line().0..=grid.bottommost_line().0)
        .rev()
        .flat_map(|line| (0..term.columns()).rev().map(move |column| (line, column)));
    let end = (back.clone())
        .filter(|&at| at < cursor)
        .find(|&at| prompted(cell(at)));
    // The newest line typed, by its number, which only grows, its first and last cells; a shell drawing its prompts again keeps it.
    let typed = back.filter_map(|at| match part(cell(at)) {
        Some(Part::Input(line)) => Some((at, line)),
        _ => None,
    });
    let newest = typed.fold(None, |newest, (at, line)| match newest {
        Some((_, _, kept)) if kept > line => newest,
        Some((_, last, kept)) if kept == line => Some((at, last, line)),
        _ => Some((at, at, line)),
    });
    let input = newest.map(|(first, _, line)| (first, line));
    let bottom = newest.map_or(cursor.0, |(_, last, _)| last.0.max(cursor.0));
    // An older line typed between a prompt and the input leaves that prompt behind.
    let older = |end: (i32, usize), (input, line): ((i32, usize), u32)| {
        (end.0..=input.0)
            .flat_map(|row| {
                let from = if row == end.0 { end.1 + 1 } else { 0 };
                let to = if row == input.0 {
                    input.1
                } else {
                    term.columns()
                };
                (from..to).map(move |column| (row, column))
            })
            .any(|at| matches!(part(cell(at)), Some(Part::Input(other)) if other != line))
    };
    let at = (click.line.0, click.column.0);
    // Input starts past the prompt's last cell before the cursor, or at its first typed cell where no prompt of its own came before it.
    let (before, first) = match (end, input) {
        (Some(end), Some(typed)) if typed.0 > end && older(end, typed) => (at < typed.0, typed.0.0),
        (Some(end), _) => (at <= end, start(term, end.0)?),
        (None, Some((input, _))) => (at < input, input.0),
        (None, None) => return None,
    };
    if before || at.0 > bottom || prompted(cell(at)) {
        return None;
    }
    let row = if relative { at.0 - first } else { at.0 } + 1;
    if row < 1 {
        return None;
    }
    Some(format!("\x1b[<0;{};{row}M", click.column.0 + 1).into_bytes())
}

impl TerminalView {
    /// Moves the shell's cursor to `click` while it reads a line: arrows, each pressed and let go, or the press where the shell takes clicks.
    pub(super) fn click_to_move(&mut self, click: Point, cx: &mut Context<Self>) {
        let pid = self.pty.pid;
        let Shell::Reading(takes) = self.shell else {
            log::debug!("terminal {pid}: a click at {click:?}, no line read");
            return;
        };
        if self.mouse.tracking != Tracking::Off {
            log::debug!("terminal {pid}: a click at {click:?}, the program tracks the mouse");
            return;
        }
        if takes != Click::Arrows {
            let Some(bytes) = press(&self.pty.term.lock(), click, takes == Click::Relative) else {
                log::debug!("terminal {pid}: a click at {click:?}, off the input");
                return;
            };
            log::debug!("terminal {pid}: a click at {click:?} goes to the shell as {bytes:?}");
            return self.input(bytes, cx);
        }
        let Some(count) = arrows(&self.pty.term.lock(), click) else {
            log::debug!("terminal {pid}: a click at {click:?}, off the input");
            return;
        };
        log::debug!("terminal {pid}: a click at {click:?} moves the cursor {count}");
        let arrow = if count < 0 { "left" } else { "right" };
        for _ in 0..count.unsigned_abs() {
            self.tap_key(arrow, cx);
        }
    }
}

#[cfg(test)]
mod tests;
