/// A semantic prompt mark a program sends, OSC 133.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// A prompt starts: A, or N.
    Prompt,
    /// The prompt ends and input starts: B, or I.
    Input,
    /// Input ends and the command's output starts: C.
    Output,
    /// The command ended, with the status it gave: D.
    Done(Option<i32>),
}

/// The mark in an OSC 133 body past `133;`; none for one the terminal takes no action on.
pub(crate) fn read(body: &[u8]) -> Option<Mark> {
    let shown = || String::from_utf8_lossy(body);
    let Some((&letter, rest)) = body.split_first() else {
        log::warn!("pty: OSC 133 without a mark");
        return None;
    };
    let options = match rest {
        [] => rest,
        [b';', options @ ..] => options,
        _ => {
            log::warn!("pty: OSC 133;{} is no mark", shown());
            return None;
        }
    };
    match letter {
        b'A' | b'N' => Some(Mark::Prompt),
        b'B' | b'I' => Some(Mark::Input),
        b'C' => Some(Mark::Output),
        b'D' => Some(Mark::Done(status(options))),
        // L's fresh line needs the cursor, unseen before the parser; P adds nothing to A.
        b'L' | b'P' => {
            log::debug!("pty: OSC 133;{} takes no action here", shown());
            None
        }
        _ => {
            log::warn!("pty: OSC 133;{} is no mark", shown());
            None
        }
    }
}

/// D's status, its first option; none when the program gave none or no number.
fn status(options: &[u8]) -> Option<i32> {
    let first = options.split(|&byte| byte == b';').next()?;
    if first.is_empty() {
        return None;
    }
    let status = std::str::from_utf8(first)
        .ok()
        .and_then(|text| text.parse().ok());
    if status.is_none() {
        log::warn!(
            "pty: OSC 133;D with a status that is no number ({})",
            String::from_utf8_lossy(first)
        );
    }
    status
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_mark_reads_with_its_options_as_ghostty_reads_it() {
        assert_eq!(read(b"A"), Some(Mark::Prompt));
        assert_eq!(read(b"A;click_events=1"), Some(Mark::Prompt));
        assert_eq!(read(b"N;aid=7"), Some(Mark::Prompt));
        assert_eq!(read(b"B"), Some(Mark::Input));
        assert_eq!(read(b"I"), Some(Mark::Input));
        assert_eq!(read(b"C;cmdline_url=false"), Some(Mark::Output));
        assert_eq!(read(b"D;1"), Some(Mark::Done(Some(1))));
        assert_eq!(read(b"D;127;aid=foo"), Some(Mark::Done(Some(127))));
        assert_eq!(read(b"D;-1"), Some(Mark::Done(Some(-1))));
        assert_eq!(read(b"D"), Some(Mark::Done(None)));
        assert_eq!(read(b"D;"), Some(Mark::Done(None)));
        assert_eq!(read(b"D;abc"), Some(Mark::Done(None)));
    }

    #[test]
    fn a_mark_the_terminal_takes_no_action_on_reads_as_none() {
        for body in [&b""[..], b"L", b"P;k=s", b"AB", b"Q", b"a"] {
            assert_eq!(read(body), None, "{}", String::from_utf8_lossy(body));
        }
    }
}
