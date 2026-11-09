use std::{
    ops::{Range, RangeInclusive},
    path::{Path, PathBuf},
};

use alacritty_terminal::{
    grid::Dimensions,
    index::{Boundary, Column, Line, Point},
    term::{
        Term,
        cell::{Cell, Flags},
    },
};
use ely_gpui_component::terminal::{Target, links};

use super::{prompts, tap::folder::file_uri, view::TerminalView};

/// Auto-detected links need a complete token within both budgets.
const URL_CELLS: usize = 4096;
const URL_BYTES: usize = 16 * 1024;

/// What a press opens.
pub enum Link {
    Url(String),
    File(PathBuf),
}

impl TerminalView {
    /// The link at `point`; a path counts only where it names a file on disk.
    pub(super) fn link(&self, point: Point) -> Option<Link> {
        let (target, word) = {
            let term = self.pty.term.lock();
            (link_at(&term, point), word_at(&term, point))
        };
        let folder = self.folder.as_deref();
        let spoken = || word.as_deref().and_then(|word| named(word, folder));
        match target {
            Some(Target::Url(url)) => {
                Some(local(&url).map_or_else(|| Link::Url(url.to_string()), Link::File))
            }
            Some(Target::Path { path, .. }) => file(&path, folder).or_else(spoken).map(Link::File),
            None => spoken().map(Link::File),
        }
    }

    /// The file a right press names: the selection it lands in, else the link at `point`.
    pub(super) fn file_at(&self, point: Point) -> Option<PathBuf> {
        let selected = {
            let term = self.pty.term.lock();
            let range = term.selection.as_ref().and_then(|it| it.to_range(&term));
            range
                .is_some_and(|range| range.contains(point))
                .then(|| term.selection_to_string())
                .flatten()
        };
        let folder = self.folder.as_deref();
        selected
            .and_then(|text| named(&text, folder))
            .or_else(|| match self.link(point)? {
                Link::File(path) => Some(path),
                Link::Url(_) => None,
            })
    }
}

/// `path` on disk: `~/` from home, a relative one from `folder`.
fn file(path: &str, folder: Option<&Path>) -> Option<PathBuf> {
    let full = match path.strip_prefix("~/") {
        Some(rest) => dirs::home_dir()?.join(rest),
        None if Path::new(path).is_absolute() => PathBuf::from(path),
        None => folder?.join(path),
    };
    full.exists().then_some(full)
}

/// The file on this machine a `file://` URL names, while it is there.
fn local(url: &str) -> Option<PathBuf> {
    url.starts_with("file://")
        .then(|| file_uri(url.as_bytes()))
        .flatten()
        .filter(|path| path.exists())
}

/// The file loose text names, past what surrounds a path in prose.
fn named(text: &str, folder: Option<&Path>) -> Option<PathBuf> {
    let text = text.trim();
    (!text.contains('\n')).then(|| file(bare(text)?, folder))?
}

/// `text` without the quotes, brackets, punctuation and `:line:column` around its path.
fn bare(text: &str) -> Option<&str> {
    let mut name = text
        .trim_start_matches(['"', '\'', '`', '(', '[', '{', '<'])
        .trim_end_matches([
            '"', '\'', '`', ')', ']', '}', '>', ',', ';', ':', '.', '!', '?',
        ]);
    for _ in 0..2 {
        if let Some((head, tail)) = name.rsplit_once(':')
            && !tail.is_empty()
            && tail.bytes().all(|byte| byte.is_ascii_digit())
        {
            name = head;
        }
    }
    name.chars().any(char::is_alphanumeric).then_some(name)
}

fn character_at<T>(term: &Term<T>, mut point: Point) -> Point {
    if term.grid()[point].flags.contains(Flags::WIDE_CHAR_SPACER) {
        point.column -= 1;
    }
    point
}

pub(super) fn hit_at<T>(term: &Term<T>, point: Point) -> Option<RangeInclusive<Point>> {
    let grid = term.grid();
    let top = Line(-(grid.display_offset() as i32));
    let first_visible = Point::new(top, Column(0));
    let last_visible = Point::new(top + term.screen_lines() - 1, term.last_column());
    if !(first_visible..=last_visible).contains(&point) {
        return None;
    }
    let point = character_at(term, point);
    if let Some(link) = grid[point]
        .hyperlink()
        .filter(|link| link.uri() != prompts::LINK)
    {
        let (mut first, mut last) = (point, point);
        while first > first_visible {
            let previous = first.sub(term, Boundary::Grid, 1);
            if grid[previous].hyperlink().as_ref() != Some(&link) {
                break;
            }
            first = previous;
        }
        while last < last_visible {
            let next = last.add(term, Boundary::Grid, 1);
            if grid[next].hyperlink().as_ref() != Some(&link) {
                break;
            }
            last = next;
        }
        return Some(first..=last);
    }
    let (bytes, mut cell) = url_at(term, point)?;
    let (mut byte, mut first) = (0, None);
    for _ in 0..URL_CELLS {
        let content = &grid[cell];
        if !spacer(content) {
            let end = byte
                + content.c.len_utf8()
                + content
                    .zerowidth()
                    .into_iter()
                    .flatten()
                    .filter(|ch| **ch != '\u{200b}')
                    .map(|ch| ch.len_utf8())
                    .sum::<usize>();
            if end > bytes.start {
                first.get_or_insert(cell);
            }
            if end >= bytes.end {
                let last = if content.flags.contains(Flags::WIDE_CHAR) {
                    cell.add(term, Boundary::Grid, 1)
                } else {
                    cell
                };
                return Some(first?..=last);
            }
            byte = end;
        }
        cell = cell.add(term, Boundary::Grid, 1);
    }
    None
}

fn spacer(cell: &Cell) -> bool {
    cell.flags
        .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
}

fn token<T>(term: &Term<T>, point: Point) -> Option<RangeInclusive<Point>> {
    let grid = term.grid();
    let separator = |point: Point| {
        let cell = &grid[point];
        !spacer(cell) && (cell.c.is_whitespace() || matches!(cell.c, '<' | '>' | '"' | '\'' | '`'))
    };
    if separator(point) {
        return None;
    }
    let (mut first, mut last, mut count) = (point, point, 1);
    loop {
        let previous = first.sub(term, Boundary::Grid, 1);
        if previous == first
            || (first.column == Column(0) && !grid[previous].flags.contains(Flags::WRAPLINE))
            || separator(previous)
        {
            break;
        }
        if count == URL_CELLS {
            return None;
        }
        first = previous;
        count += 1;
    }
    loop {
        let next = last.add(term, Boundary::Grid, 1);
        if next == last
            || (last.column == term.last_column() && !grid[last].flags.contains(Flags::WRAPLINE))
            || separator(next)
        {
            break;
        }
        if count == URL_CELLS {
            return None;
        }
        last = next;
        count += 1;
    }
    Some(first..=last)
}

/// The text of `point`'s token, where in it `point` falls, and the token's first cell.
fn token_text<T>(term: &Term<T>, point: Point) -> Option<(String, usize, Point)> {
    let grid = term.grid();
    let token = token(term, point)?;
    let mut cell = *token.start();
    let (mut text, mut at) = (String::new(), 0);
    loop {
        if cell == point {
            at = text.len();
        }
        if !spacer(&grid[cell]) {
            // A zero-width space fences a URL, as terminal-link's fallback writes it.
            let marks = grid[cell].zerowidth().into_iter().flatten();
            for ch in
                std::iter::once(&grid[cell].c).chain(marks.filter(|&&mark| mark != '\u{200b}'))
            {
                if text.len() + ch.len_utf8() > URL_BYTES {
                    return None;
                }
                text.push(*ch);
            }
        }
        if cell == *token.end() {
            break;
        }
        cell = cell.add(term, Boundary::Grid, 1);
    }
    Some((text, at, *token.start()))
}

/// The OSC 8 target at `point`, else a URL or a path in its wrapped line; a prompt's own link is no target.
pub fn link_at<T>(term: &Term<T>, point: Point) -> Option<Target> {
    let point = character_at(term, point);
    if let Some(link) = term.grid()[point]
        .hyperlink()
        .filter(|link| link.uri() != prompts::LINK)
    {
        return Some(Target::Url(link.uri().to_string().into()));
    }
    let (text, at, _) = token_text(term, point)?;
    links(&text)
        .into_iter()
        .find_map(|(bytes, target)| bytes.contains(&at).then_some(target))
}

/// Where the URL under `point` lies in its token's text, and the token's first cell.
fn url_at<T>(term: &Term<T>, point: Point) -> Option<(Range<usize>, Point)> {
    let (text, at, start) = token_text(term, point)?;
    links(&text)
        .into_iter()
        .find_map(|(bytes, target)| match target {
            Target::Url(_) if bytes.contains(&at) => Some((bytes, start)),
            _ => None,
        })
}

/// The run of non-blank text at `point` in its wrapped line.
fn word_at<T>(term: &Term<T>, point: Point) -> Option<String> {
    let (text, at, _) = token_text(term, character_at(term, point))?;
    word(&text, at).map(str::to_string)
}

/// The run of non-blank text over byte `at`; a blank is no word.
fn word(text: &str, at: usize) -> Option<&str> {
    if text[at..].chars().next()?.is_whitespace() {
        return None;
    }
    let start = text[..at].rfind(char::is_whitespace).map_or(0, |blank| {
        blank + text[blank..].chars().next().map_or(0, char::len_utf8)
    });
    let end = text[at..]
        .find(char::is_whitespace)
        .map_or(text.len(), |blank| at + blank);
    Some(&text[start..end])
}

#[cfg(test)]
mod tests;
