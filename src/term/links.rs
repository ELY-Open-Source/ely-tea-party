use std::path::{Path, PathBuf};

use alacritty_terminal::{
    index::{Boundary, Point},
    term::{Term, cell::Flags},
};
use ely_gpui_component::terminal::{Target, links};

use super::{prompts, view::TerminalView};

/// What a press opens.
pub enum Link {
    Url(String),
    File(PathBuf),
}

impl TerminalView {
    /// The link at `point`; a path counts only where it names a file on disk.
    pub(super) fn link(&self, point: Point) -> Option<Link> {
        let target = link_at(&self.pty.term.lock(), point)?;
        match target {
            Target::Url(url) => Some(Link::Url(url.to_string())),
            Target::Path { path, .. } => file(&path, self.folder.as_deref()).map(Link::File),
        }
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

/// The OSC 8 target at `point`, else a URL or a path in its wrapped line; a prompt's own link is no target.
pub fn link_at<T>(term: &Term<T>, mut point: Point) -> Option<Target> {
    let grid = term.grid();
    if grid[point].flags.contains(Flags::WIDE_CHAR_SPACER) {
        point.column -= 1;
    }
    if let Some(link) = grid[point]
        .hyperlink()
        .filter(|link| link.uri() != prompts::LINK)
    {
        return Some(Target::Url(link.uri().to_string().into()));
    }
    let end = term.line_search_right(point);
    let mut cell = term.line_search_left(point);
    let (mut text, mut at) = (String::new(), 0);
    loop {
        if cell == point {
            at = text.len();
        }
        let spacer = Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER;
        if !grid[cell].flags.intersects(spacer) {
            text.push(grid[cell].c);
            // A zero-width space fences a URL, as terminal-link's fallback writes it.
            let marks = grid[cell].zerowidth().into_iter().flatten();
            text.extend(marks.filter(|&&mark| mark != '\u{200b}'));
        }
        if cell == end {
            break;
        }
        cell = cell.add(term, Boundary::Grid, 1);
    }
    links(&text)
        .into_iter()
        .find_map(|(bytes, target)| bytes.contains(&at).then_some(target))
}

#[cfg(test)]
mod tests {
    use alacritty_terminal::{
        event::VoidListener,
        index::{Column, Line},
        term::{Config, test::TermSize},
        vte::ansi::Processor,
    };

    use super::*;

    fn link(bytes: &str, line: i32, column: usize) -> Option<String> {
        let mut term = Term::new(Config::default(), &TermSize::new(12, 4), VoidListener);
        let mut parser: Processor = Processor::new();
        parser.advance(&mut term, bytes.as_bytes());
        link_at(&term, Point::new(Line(line), Column(column))).map(|target| match target {
            Target::Url(url) => url.to_string(),
            Target::Path { path, .. } => path.to_string(),
        })
    }

    #[test]
    fn a_path_names_its_file_without_the_line() {
        assert_eq!(link("at src/a.rs:4:2", 0, 6).as_deref(), Some("src/a.rs"));
        assert_eq!(link("at src/a.rs:4:2", 0, 1), None);
    }

    #[test]
    fn a_path_opens_only_where_a_file_is() {
        let folder = std::env::temp_dir().join(format!("ely-links-{}", std::process::id()));
        std::fs::create_dir_all(folder.join("src")).unwrap();
        std::fs::write(folder.join("src/a.rs"), "").unwrap();
        assert_eq!(
            file("src/a.rs", Some(&folder)),
            Some(folder.join("src/a.rs"))
        );
        assert_eq!(file("src/b.rs", Some(&folder)), None, "no such file");
        assert_eq!(file("src/a.rs", None), None, "no folder to start from");
        let absolute = folder.join("src/a.rs");
        assert_eq!(file(absolute.to_str().unwrap(), None), Some(absolute));
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn an_osc_8_link_names_its_target() {
        let text = "a \x1b]8;;https://ely.dev/x\x1b\\ely\x1b]8;;\x1b\\ b";
        assert_eq!(link(text, 0, 3).as_deref(), Some("https://ely.dev/x"));
        assert_eq!(link(text, 0, 0), None);
    }

    #[test]
    fn a_prompt_s_own_link_is_no_target() {
        let text = "\x1b]8;;\x7f\x1b\\$ https://ely.dev\x1b]8;;\x1b\\";
        assert_eq!(link(text, 0, 0), None);
        assert_eq!(link(text, 0, 4).as_deref(), Some("https://ely.dev"));
    }

    #[test]
    fn a_url_ends_before_its_sentence_does() {
        assert_eq!(link("(a.io/x).", 0, 2), None, "no scheme, no link");
        let text = "(http://a.b)";
        assert_eq!(link(text, 0, 4).as_deref(), Some("http://a.b"));
        assert_eq!(link(text, 0, 11), None);
    }

    #[test]
    fn a_url_keeps_its_combining_marks() {
        let text = "https://e.test/cafe\u{301}/x";
        assert_eq!(link(text, 1, 1).as_deref(), Some(text));
    }

    #[test]
    fn a_url_runs_across_wrapped_rows_and_wide_cells() {
        let text = "\u{4e2d} https://ely.dev/abc";
        assert_eq!(link(text, 1, 3).as_deref(), Some("https://ely.dev/abc"));
        assert_eq!(link(text, 0, 3).as_deref(), Some("https://ely.dev/abc"));
        assert_eq!(link(text, 0, 1), None);
        let wide = "file://a/\u{4e2d} x";
        assert_eq!(
            link(wide, 0, 10).as_deref(),
            Some("file://a/\u{4e2d}"),
            "a wide cell's spacer is its char"
        );
    }

    #[test]
    fn a_zero_width_space_ends_a_url() {
        let text = "doc (\u{200b}https://a.io/x\u{200b})";
        assert_eq!(link(text, 0, 8).as_deref(), Some("https://a.io/x"));
    }
}
