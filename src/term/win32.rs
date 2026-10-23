//! win32-input-mode, which ConPTY asks for: a key goes as Windows' own record, as Windows Terminal sends it.

use gpui::Keystroke;
use windows_sys::Win32::{
    System::Console::{
        CAPSLOCK_ON, ENHANCED_KEY, LEFT_ALT_PRESSED, LEFT_CTRL_PRESSED, NUMLOCK_ON,
        RIGHT_ALT_PRESSED, RIGHT_CTRL_PRESSED, SCROLLLOCK_ON, SHIFT_PRESSED,
    },
    UI::Input::KeyboardAndMouse::{
        GetKeyState, GetKeyboardState, MAPVK_VK_TO_CHAR, MAPVK_VK_TO_VSC, MapVirtualKeyW,
        ToUnicode, VK_APPS, VK_BACK, VK_BROWSER_BACK, VK_BROWSER_FORWARD, VK_CAPITAL, VK_DELETE,
        VK_DOWN, VK_END, VK_ESCAPE, VK_F1, VK_HOME, VK_INSERT, VK_LCONTROL, VK_LEFT, VK_LMENU,
        VK_NEXT, VK_NUMLOCK, VK_PRIOR, VK_RCONTROL, VK_RETURN, VK_RIGHT, VK_RMENU, VK_SCROLL,
        VK_SHIFT, VK_SPACE, VK_TAB, VK_UP, VkKeyScanW,
    },
};

use super::keys::Action;

/// The mode ConPTY sets, `CSI ? 9001 h`.
pub const MODE: u32 = 9001;

/// `keystroke` as ConPTY reads it, `CSI Vk ; Sc ; Uc ; Kd ; Cs ; Rc _`; none for a key no virtual key names.
pub fn record(keystroke: &Keystroke, action: Action) -> Option<Vec<u8>> {
    let (key, enhanced) = virtual_key(&keystroke.key)?;
    let scan = unsafe { MapVirtualKeyW(u32::from(key), MAPVK_VK_TO_VSC) };
    let mut state = [0u8; 256];
    let mut unit = [0u16; 2];
    // Flag 4 leaves the keyboard's dead key as it is.
    let typed = unsafe {
        if GetKeyboardState(state.as_mut_ptr()) == 0 {
            log::error!("windows: the keyboard state did not read");
            return None;
        }
        ToUnicode(
            u32::from(key),
            scan,
            state.as_ptr(),
            unit.as_mut_ptr(),
            unit.len() as i32,
            4,
        )
    };
    let unit = if typed == 1 { unit[0] } else { 0 };
    let down = u8::from(action != Action::Release);
    let control = control(enhanced);
    Some(format!("\x1b[{key};{scan};{unit};{down};{control};1_").into_bytes())
}

/// The key's own character, where gpui named `made`, what Shift made of it, and dropped Shift.
pub fn unshifted(made: &str) -> Option<char> {
    let mut chars = made.chars();
    let (Some(made), None) = (chars.next(), chars.next()) else {
        return None;
    };
    if unsafe { GetKeyState(i32::from(VK_SHIFT)) } >= 0 {
        return None;
    }
    let mut buffer = [0u16; 2];
    let &mut [unit] = made.encode_utf16(&mut buffer) else {
        return None;
    };
    // The high byte names the modifiers the character takes: here Shift alone.
    let scan = unsafe { VkKeyScanW(unit) } as u16;
    if scan == u16::MAX || scan >> 8 != 1 {
        return None;
    }
    let own = unsafe { MapVirtualKeyW(u32::from(scan & 0xFF), MAPVK_VK_TO_CHAR) } & 0xFFFF;
    char::from_u32(own)
        .map(|own| own.to_ascii_lowercase())
        .filter(|own| !own.is_control())
}

/// The virtual key gpui's `name` stands for, and whether it sits in the enhanced block.
pub fn virtual_key(name: &str) -> Option<(u16, bool)> {
    let named = match name {
        "enter" => (VK_RETURN, false),
        "tab" => (VK_TAB, false),
        "escape" => (VK_ESCAPE, false),
        "backspace" => (VK_BACK, false),
        "space" => (VK_SPACE, false),
        "menu" => (VK_APPS, true),
        "back" => (VK_BROWSER_BACK, true),
        "forward" => (VK_BROWSER_FORWARD, true),
        "left" => (VK_LEFT, true),
        "up" => (VK_UP, true),
        "right" => (VK_RIGHT, true),
        "down" => (VK_DOWN, true),
        "home" => (VK_HOME, true),
        "end" => (VK_END, true),
        "pageup" => (VK_PRIOR, true),
        "pagedown" => (VK_NEXT, true),
        "insert" => (VK_INSERT, true),
        "delete" => (VK_DELETE, true),
        name => {
            if let Some(number) = name
                .strip_prefix('f')
                .and_then(|number| number.parse::<u16>().ok())
                .filter(|number| (1..=24).contains(number))
            {
                return Some((VK_F1 + number - 1, false));
            }
            let mut chars = name.chars();
            let (Some(ch), None) = (chars.next(), chars.next()) else {
                return None;
            };
            let mut buffer = [0u16; 2];
            let &mut [unit] = ch.encode_utf16(&mut buffer) else {
                return None;
            };
            let scan = unsafe { VkKeyScanW(unit) } as u16;
            return (scan != u16::MAX).then_some((scan & 0xFF, false));
        }
    };
    Some(named)
}

/// Windows' control key state now: the modifier keys, the locks, and the enhanced block.
fn control(enhanced: bool) -> u32 {
    let down = |key: u16| unsafe { GetKeyState(i32::from(key)) } < 0;
    let on = |key: u16| unsafe { GetKeyState(i32::from(key)) } & 1 != 0;
    [
        (down(VK_SHIFT), SHIFT_PRESSED),
        (down(VK_LCONTROL), LEFT_CTRL_PRESSED),
        (down(VK_RCONTROL), RIGHT_CTRL_PRESSED),
        (down(VK_LMENU), LEFT_ALT_PRESSED),
        (down(VK_RMENU), RIGHT_ALT_PRESSED),
        (on(VK_CAPITAL), CAPSLOCK_ON),
        (on(VK_NUMLOCK), NUMLOCK_ON),
        (on(VK_SCROLL), SCROLLLOCK_ON),
        (enhanced, ENHANCED_KEY),
    ]
    .into_iter()
    .filter(|(held, _)| *held)
    .fold(0, |state, (_, flag)| state | flag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_keys_and_letters_find_their_virtual_keys() {
        assert_eq!(virtual_key("enter"), Some((VK_RETURN, false)));
        assert_eq!(virtual_key("left"), Some((VK_LEFT, true)));
        assert_eq!(virtual_key("f12"), Some((VK_F1 + 11, false)));
        assert_eq!(virtual_key("f25"), None);
        assert_eq!(virtual_key("a"), Some((u16::from(b'A'), false)));
    }

    #[test]
    fn a_record_names_the_key_its_scan_code_and_its_press() {
        let key = Keystroke::parse("enter").expect("a keystroke");
        let text =
            |action| String::from_utf8(record(&key, action).expect("a record")).expect("text");
        let scan = unsafe { MapVirtualKeyW(u32::from(VK_RETURN), MAPVK_VK_TO_VSC) };
        // The fields are the key, the scan code, the character, the press, the control state and the record's own mark.
        let fields = |record: String| {
            record
                .trim_start_matches('\x1b')
                .trim_start_matches('[')
                .trim_end_matches('_')
                .split(';')
                .map(str::to_string)
                .collect::<Vec<String>>()
        };
        let down = fields(text(Action::Press));
        assert_eq!(down[0], "13", "the key is the return key: {down:?}");
        assert_eq!(down[1], scan.to_string(), "its scan code: {down:?}");
        assert_eq!(down[3], "1", "it is a press: {down:?}");
        assert_eq!(down[5], "1", "the record's own mark: {down:?}");
        let up = fields(text(Action::Release));
        assert_eq!(up[0], "13", "the key is the return key: {up:?}");
        assert_eq!(up[1], scan.to_string(), "its scan code: {up:?}");
        assert_eq!(up[3], "0", "it is a release: {up:?}");
    }
}
