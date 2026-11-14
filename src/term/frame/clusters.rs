//! Emoji sequences a program spread over cells, read back as the one cluster a font draws.

use alacritty_terminal::{
    grid::Dimensions,
    index::{Column, Point},
    term::{
        Term,
        cell::{Cell, Flags},
    },
};

/// Joins the emoji on either side of it into one.
const JOINER: char = '\u{200D}';

/// A skin tone, which colors the emoji before it.
fn tone(c: char) -> bool {
    matches!(c, '\u{1F3FB}'..='\u{1F3FF}')
}

/// Whether `c` may take a tone or a joiner: pictographs, not ideographs, which are wide too.
pub(super) fn pictograph(c: char) -> bool {
    matches!(c, '\u{2600}'..='\u{27BF}' | '\u{1F000}'..='\u{1FAFF}')
}

fn push(text: &mut String, cell: &Cell) {
    text.push(cell.c);
    text.extend(cell.zerowidth().into_iter().flatten());
}

/// The sequence the pictograph at `point` starts with the cells after it, and the column it ends before; none when the cell stands alone.
pub(super) fn sequence<T>(term: &Term<T>, point: Point) -> Option<(String, usize)> {
    let row = &term.grid()[point.line];
    let width = |cell: &Cell| 1 + usize::from(cell.flags.contains(Flags::WIDE_CHAR));
    let first = &row[point.column];
    let mut text = String::new();
    push(&mut text, first);
    let start = point.column.0 + width(first);
    let mut end = start;
    while end < term.columns() {
        let next = &row[Column(end)];
        // A joiner takes whatever follows it; a tone is wide, as the emoji it colors.
        let joins = text.ends_with(JOINER) || (tone(next.c) && width(first) == 2);
        if !joins || next.c == ' ' || next.flags.contains(Flags::HIDDEN) {
            break;
        }
        push(&mut text, next);
        end += width(next);
    }
    (end > start).then_some((text, end))
}

#[cfg(test)]
mod tests {
    use alacritty_terminal::{
        event::VoidListener,
        term::{Config, test::TermSize},
        vte::ansi::Processor,
    };

    use super::super::{Frame, frame};
    use super::*;
    use crate::{term::ink::Ink, themes};

    fn shown(text: &str) -> Frame {
        let mut term = Term::new(Config::default(), &TermSize::new(20, 2), VoidListener);
        let mut parser: Processor = Processor::new();
        parser.advance(&mut term, text.as_bytes());
        let ink = Ink::new(themes::find("dracula").expect("dracula"), term.colors());
        frame(&term, &ink, &[], None)
    }

    fn alone(frame: &Frame) -> Vec<(usize, &str)> {
        frame.rows[0]
            .alone
            .iter()
            .map(|(at, text, _)| (*at, text.as_str()))
            .collect()
    }

    #[test]
    fn a_skin_tone_joins_the_emoji_before_it_and_leaves_its_cells_blank() {
        let frame = shown("a\u{1F44D}\u{1F3FD}b");
        assert_eq!(alone(&frame), [(1, "\u{1F44D}\u{1F3FD}")]);
        assert_eq!(&frame.rows[0].text[..6], "a    b");
    }

    #[test]
    fn joiners_gather_a_family_into_its_first_cell() {
        let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        let frame = shown(&format!("{family}x"));
        assert_eq!(alone(&frame), [(0, family)]);
        assert_eq!(&frame.rows[0].text[..7], "      x");
    }

    #[test]
    fn a_joiner_takes_a_narrow_sign_and_a_narrow_base_takes_a_wide_emoji() {
        let woman = "\u{1F64B}\u{1F3FF}\u{200D}\u{2640}\u{FE0F}";
        let frame = shown(&format!("{woman}x"));
        assert_eq!(alone(&frame), [(0, woman)]);
        assert_eq!(&frame.rows[0].text[..6], "     x");
        let heart = "\u{2764}\u{FE0F}\u{200D}\u{1F525}";
        let frame = shown(&format!("{heart}x"));
        assert_eq!(alone(&frame), [(0, heart)]);
        assert_eq!(&frame.rows[0].text[..4], "   x");
    }

    #[test]
    fn emoji_side_by_side_and_ideographs_stay_in_the_row() {
        let frame = shown("\u{1F44D}\u{1F389}\u{4E2D}\u{1F3FD}");
        assert!(alone(&frame).is_empty());
        assert!(
            frame.rows[0]
                .text
                .starts_with("\u{1F44D} \u{1F389} \u{4E2D} \u{1F3FD}")
        );
    }

    #[test]
    fn the_cursor_on_a_sequence_reads_it_whole() {
        let frame = shown("\u{1F44D}\u{1F3FD}\x1b[4D");
        assert_eq!(frame.cursor.expect("a cursor").text, "\u{1F44D}\u{1F3FD}");
    }
}
