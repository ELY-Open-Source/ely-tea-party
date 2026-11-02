use std::path::{Path, PathBuf};

use alacritty_terminal::{
    index::{Boundary, Point},
    term::{Term, cell::Flags},
};
use ely_gpui_component::terminal::{Target, links};

use super::{prompts, tap::folder::file_uri, view::TerminalView};

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

/// The cells of `point`'s wrapped line as text, and where in it `point` falls.
fn line_at<T>(term: &Term<T>, point: Point) -> (String, usize) {
    let grid = term.grid();
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
    (text, at)
}

/// `point` on a wide character's spacer stands for the character.
fn on_char<T>(term: &Term<T>, mut point: Point) -> Point {
    if term.grid()[point].flags.contains(Flags::WIDE_CHAR_SPACER) {
        point.column -= 1;
    }
    point
}

/// The OSC 8 target at `point`, else a URL or a path in its wrapped line; a prompt's own link is no target.
pub fn link_at<T>(term: &Term<T>, point: Point) -> Option<Target> {
    let point = on_char(term, point);
    if let Some(link) = term.grid()[point]
        .hyperlink()
        .filter(|link| link.uri() != prompts::LINK)
    {
        return Some(Target::Url(link.uri().to_string().into()));
    }
    let (text, at) = line_at(term, point);
    links(&text)
        .into_iter()
        .find_map(|(bytes, target)| bytes.contains(&at).then_some(target))
}

/// The run of non-blank text at `point` in its wrapped line.
fn word_at<T>(term: &Term<T>, point: Point) -> Option<String> {
    let (text, at) = line_at(term, on_char(term, point));
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
    fn a_word_is_the_run_of_text_under_the_point() {
        let text = "see 中文/a.rs now";
        assert_eq!(word(text, 4), Some("中文/a.rs"));
        assert_eq!(word(text, text.find("a.rs").unwrap()), Some("中文/a.rs"));
        assert_eq!(word(text, 3), None, "a blank is no word");
        assert_eq!(word(text, text.len()), None, "past the text");
        assert_eq!(word("a\u{3000}b", 4), Some("b"), "a wide blank ends it");
    }

    #[test]
    fn a_word_loses_what_surrounds_its_path() {
        assert_eq!(bare("(src/a.rs:4:2),"), Some("src/a.rs"));
        assert_eq!(bare("\"README.md\"."), Some("README.md"));
        assert_eq!(bare("src/term/"), Some("src/term/"));
        assert_eq!(bare(".env"), Some(".env"));
        assert_eq!(bare("../a"), Some("../a"));
        for mark in ["/", "..", "~/", "--", ":4"] {
            assert_eq!(bare(mark), None, "{mark} names no file");
        }
    }

    #[test]
    fn loose_text_names_what_the_pattern_does_not() {
        let folder = std::env::temp_dir().join(format!("ely-named-{}", std::process::id()));
        std::fs::create_dir_all(folder.join("my docs/src")).unwrap();
        std::fs::write(folder.join("Makefile"), "").unwrap();
        std::fs::write(folder.join("my docs/a.txt"), "").unwrap();
        let here = Some(folder.as_path());
        assert_eq!(named("Makefile:3", here), Some(folder.join("Makefile")));
        assert_eq!(named("my docs/src", here), Some(folder.join("my docs/src")));
        assert_eq!(
            named("  'my docs/a.txt'\n", here),
            Some(folder.join("my docs/a.txt")),
            "a selection's own blanks go"
        );
        assert_eq!(named("Makefile\nmy docs", here), None, "two lines");
        assert_eq!(named("nothing", here), None, "no such file");
        std::fs::remove_dir_all(&folder).unwrap();
    }

    #[test]
    fn a_file_url_opens_as_its_file_while_it_is_here() {
        let path = std::env::temp_dir().join(format!("ely-local {}", std::process::id()));
        std::fs::write(&path, "").unwrap();
        let name = path
            .to_str()
            .unwrap()
            .replace('\\', "/")
            .replace(' ', "%20");
        #[cfg(windows)]
        let name = format!("/{name}");
        assert_eq!(local(&format!("file://{name}")), Some(path.clone()));
        std::fs::remove_file(&path).unwrap();
        assert_eq!(local(&format!("file://{name}")), None, "no longer there");
        assert_eq!(local("file://elsewhere.test/tmp"), None, "another host's");
        assert_eq!(local("https://a.b/c"), None);
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
