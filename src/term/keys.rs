mod legacy;

use legacy::{FUNCTION_TILDES, legacy, printable_key};

/// Whether a key went down, repeated, or came up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Press,
    Repeat,
    Release,
}

/// A key as gpui names it, what it types, and the modifiers that count.
#[derive(Clone, Copy, Debug)]
pub struct Key<'a> {
    /// gpui's name: `a`, `!`, `space`, `enter`, `f5`.
    pub name: &'a str,
    /// What the key types; empty when it types nothing.
    pub text: &'a str,
    pub shift: bool,
    pub ctrl: bool,
    /// Alt as a modifier, not as the key that composes characters.
    pub alt: bool,
    /// Cmd, the Windows key or Super: kitty's super.
    pub platform: bool,
    pub action: Action,
    /// The key's own character, where gpui named the one Shift made and dropped Shift.
    pub unshifted: Option<char>,
}

/// The kitty keyboard flags a program set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Kitty {
    pub disambiguate: bool,
    pub events: bool,
    pub alternates: bool,
    pub all: bool,
    pub text: bool,
}

/// The bytes a key sends; none when it sends nothing or its text is the input method's to type.
pub fn encode(key: &Key, app_cursor: bool, kitty: Kitty) -> Option<Vec<u8>> {
    let on = kitty.disambiguate || kitty.events || kitty.alternates || kitty.all || kitty.text;
    if !on {
        return legacy(key, app_cursor);
    }
    if key.action == Action::Release && !kitty.events {
        return None;
    }
    match functional(key.name) {
        Some((number, end)) => named(key, number, end, app_cursor, kitty),
        None => typed(key, printable(key.name)?, kitty),
    }
}

/// Readline's bytes for macOS's editing keys; `option` is Option held, whatever Option as Alt says. The caller drops a release.
pub fn natural(key: &Key, option: bool) -> Option<&'static [u8]> {
    if key.ctrl || key.shift {
        return None;
    }
    Some(match (key.name, option, key.platform) {
        ("left", true, false) => b"\x1bb",
        ("right", true, false) => b"\x1bf",
        ("backspace", true, false) => b"\x1b\x7f",
        ("delete", true, false) => b"\x1bd",
        ("left", false, true) => b"\x01",
        ("right", false, true) => b"\x05",
        ("backspace", false, true) => b"\x15",
        ("delete", false, true) => b"\x0b",
        _ => return None,
    })
}

/// Pasted text for the program; bracketed text loses ESC and Ctrl-C, as in alacritty.
pub fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    if bracketed {
        let text = text.replace(['\x1b', '\x03'], "");
        format!("\x1b[200~{text}\x1b[201~").into_bytes()
    } else {
        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
    }
}

/// xterm's modifier code: one plus shift 1, alt 2, ctrl 4, super 8.
fn code(key: &Key) -> u32 {
    1 + u32::from(key.shift)
        + 2 * u32::from(key.alt)
        + 4 * u32::from(key.ctrl)
        + 8 * u32::from(key.platform)
}

/// The character a key name stands for, `space` included.
fn printable(name: &str) -> Option<char> {
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) => Some(ch),
        _ => (name == "space").then_some(' '),
    }
}

/// Whether the key types its text: some, none of it control, and no modifier that stops text.
pub(super) fn types(key: &Key) -> bool {
    let control = key.text.chars().any(char::is_control);
    !key.text.is_empty() && !control && !key.ctrl && !key.alt && !key.platform
}

/// The text's code points joined by colons, control codes left out.
fn code_points(text: &str) -> String {
    let points: Vec<String> = text
        .chars()
        .filter(|ch| !ch.is_control())
        .map(|ch| (ch as u32).to_string())
        .collect();
    points.join(":")
}

/// kitty's functional keys: the number and the final byte.
fn functional(name: &str) -> Option<(u32, char)> {
    Some(match name {
        "escape" => (27, 'u'),
        "enter" => (13, 'u'),
        "tab" => (9, 'u'),
        "backspace" => (127, 'u'),
        "insert" => (2, '~'),
        "delete" => (3, '~'),
        "left" => (1, 'D'),
        "right" => (1, 'C'),
        "up" => (1, 'A'),
        "down" => (1, 'B'),
        "pageup" => (5, '~'),
        "pagedown" => (6, '~'),
        "home" => (1, 'H'),
        "end" => (1, 'F'),
        "f1" => (1, 'P'),
        "f2" => (1, 'Q'),
        "f3" => (13, '~'),
        "f4" => (1, 'S'),
        name => match FUNCTION_TILDES.iter().position(|(f, _)| *f == name)? {
            index @ 0..=7 => (FUNCTION_TILDES[index].1, '~'),
            index => (57376 + index as u32 - 8, 'u'),
        },
    })
}

/// kitty's event field: 2 for a repeat, 3 for a release, none for a press or without the flag.
fn event(key: &Key, kitty: Kitty) -> Option<u32> {
    match key.action {
        Action::Press => None,
        _ if !kitty.events => None,
        Action::Repeat => Some(2),
        Action::Release => Some(3),
    }
}

/// A functional key: legacy bytes until a flag that reshapes it is on.
fn named(key: &Key, number: u32, end: char, app_cursor: bool, kitty: Kitty) -> Option<Vec<u8>> {
    if !kitty.events && !kitty.disambiguate && !kitty.all {
        return legacy(key, app_cursor);
    }
    let lone = matches!(key.name, "enter" | "tab" | "backspace");
    if key.action == Action::Release && lone && !kitty.all {
        return None;
    }
    let plain = code(key) == 1 && key.action != Action::Release && !kitty.all;
    match key.name {
        "enter" if plain => return Some(b"\r".to_vec()),
        "tab" if plain => return Some(b"\t".to_vec()),
        "backspace" if plain => return Some(vec![0x7F]),
        "escape" if plain && !kitty.disambiguate => return Some(b"\x1b".to_vec()),
        _ => {}
    }
    let mods = code(key);
    let event = event(key, kitty);
    let fields = match (mods, event) {
        (_, Some(event)) => format!("{mods}:{event}"),
        (1, None) => String::new(),
        (_, None) => mods.to_string(),
    };
    let sequence = match (end, fields.is_empty()) {
        ('u' | '~', true) => format!("\x1b[{number}{end}"),
        ('u' | '~', false) => format!("\x1b[{number};{fields}{end}"),
        (_, true) => format!("\x1b[{end}"),
        (_, false) => format!("\x1b[1;{fields}{end}"),
    };
    Some(sequence.into_bytes())
}

/// A printable key: text, then legacy bytes, then CSI u once a flag has more to say.
fn typed(key: &Key, ch: char, kitty: Kitty) -> Option<Vec<u8>> {
    let typing = types(key);
    if typing && !kitty.all && key.action != Action::Release {
        return None;
    }
    // gpui names what Shift made of a key and drops Shift; CSI u names the key, with Shift.
    let (own, shift, made) = match key.unshifted {
        Some(own) => (own, true, Some(ch).filter(|&made| made != own)),
        None => (ch, key.shift, None),
    };
    let shifted = key
        .text
        .chars()
        .next()
        .filter(|&typed| typing && typed != own && !typed.is_control())
        .or(made)
        .or_else(|| (shift && own.is_ascii_lowercase()).then(|| own.to_ascii_uppercase()))
        .filter(|_| shift && kitty.alternates);
    let event = event(key, kitty);
    let text = typing && kitty.text && key.action != Action::Release;
    let mods = code(&Key { shift, ..*key });
    if shifted.is_none() && event.is_none() && !text {
        if mods == 1 && !kitty.all {
            return None;
        }
        if mods > 1
            && !kitty.disambiguate
            && !kitty.all
            && let Some(bytes) = printable_key(key, ch)
        {
            return Some(bytes);
        }
    }
    let mut out = format!("\x1b[{}", own.to_ascii_lowercase() as u32);
    if let Some(shifted) = shifted {
        out.push_str(&format!(":{}", shifted as u32));
    }
    let second = match (mods, event) {
        (_, Some(event)) => format!("{mods}:{event}"),
        (1, None) => String::new(),
        (_, None) => mods.to_string(),
    };
    let third = if text {
        code_points(key.text)
    } else {
        String::new()
    };
    if !second.is_empty() || !third.is_empty() {
        out.push_str(&format!(";{second}"));
    }
    if !third.is_empty() {
        out.push_str(&format!(";{third}"));
    }
    out.push('u');
    Some(out.into_bytes())
}

#[cfg(test)]
mod tests;
