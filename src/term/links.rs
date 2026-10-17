use alacritty_terminal::{
    index::{Boundary, Point},
    term::{Term, cell::Flags},
};
use ely_gpui_component::terminal::{Target, links};

/// The OSC 8 target at `point`, else a URL in its wrapped line.
pub fn link_at<T>(term: &Term<T>, mut point: Point) -> Option<String> {
    let grid = term.grid();
    if grid[point].flags.contains(Flags::WIDE_CHAR_SPACER) {
        point.column -= 1;
    }
    if let Some(link) = grid[point].hyperlink() {
        return Some(link.uri().to_string());
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
        .find_map(|(bytes, target)| match target {
            Target::Url(url) if bytes.contains(&at) => Some(url.to_string()),
            _ => None,
        })
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
        link_at(&term, Point::new(Line(line), Column(column)))
    }

    #[test]
    fn an_osc_8_link_names_its_target() {
        let text = "a \x1b]8;;https://ely.dev/x\x1b\\ely\x1b]8;;\x1b\\ b";
        assert_eq!(link(text, 0, 3).as_deref(), Some("https://ely.dev/x"));
        assert_eq!(link(text, 0, 0), None);
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
