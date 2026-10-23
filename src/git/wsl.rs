//! A folder in a WSL distribution, `\\wsl.localhost\DISTRO\...`, whose git runs inside it: Windows' git reads that share slowly and refuses it as another's.

use std::path::{Path, PathBuf};

/// The share Windows names `folder`'s distribution by, `\\HOST\DISTRO`, and the folder's path inside it.
pub fn split(folder: &Path) -> Option<(&str, String)> {
    let text = folder.to_str()?;
    let host = ["\\\\wsl.localhost\\", "\\\\wsl$\\"]
        .into_iter()
        .find(|host| {
            text.get(..host.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(host))
        })?;
    let end = text[host.len()..]
        .find('\\')
        .map_or(text.len(), |at| host.len() + at);
    if end == host.len() {
        return None;
    }
    let inside = to_linux(&text[end..].replace('\\', "/"));
    Some((
        &text[..end],
        if inside.is_empty() {
            "/".into()
        } else {
            inside
        },
    ))
}

/// The distribution `share` names.
pub fn distro(share: &str) -> &str {
    share.rsplit_once('\\').map_or(share, |(_, name)| name)
}

/// A path git printed inside `share`'s distribution, as Windows names it.
pub fn join(share: &str, inside: &Path) -> PathBuf {
    let inside = to_windows(&inside.display().to_string()).replace('/', "\\");
    PathBuf::from(format!("{share}{inside}"))
}

/// Whether WSL shows `c` to Windows as U+F000 plus `c`: a control character, or one NTFS refuses.
fn escaped(c: char) -> bool {
    c < ' ' || "\"*:<>?\\|".contains(c)
}

/// A Linux name as Windows reads it through WSL's share, as WSL's `escape.cpp` writes it.
pub fn to_windows(name: &str) -> String {
    let escape = |c: char| char::from_u32(0xF000 + c as u32).expect("U+F000 to U+F07F");
    name.chars()
        .map(|c| if escaped(c) { escape(c) } else { c })
        .collect()
}

/// A name from WSL's share as Linux names it.
fn to_linux(name: &str) -> String {
    let plain = |c: char| (c as u32).checked_sub(0xF000).and_then(char::from_u32);
    name.chars()
        .map(|c| plain(c).filter(|plain| escaped(*plain)).unwrap_or(c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wsl_share_names_its_distribution_and_the_path_inside() {
        let folder = Path::new("\\\\wsl.localhost\\Ubuntu-24.04\\home\\me\\my repo\u{f03a}x");
        let (share, inside) = split(folder).expect("a WSL folder");
        assert_eq!(share, r"\\wsl.localhost\Ubuntu-24.04");
        assert_eq!(inside, "/home/me/my repo:x");
        assert_eq!(distro(share), "Ubuntu-24.04");
        let top = join(share, Path::new("/home/me/a:b\\c\u{7}"));
        let shown = "\\\\wsl.localhost\\Ubuntu-24.04\\home\\me\\a\u{f03a}b\u{f05c}c\u{f007}";
        assert_eq!(top, PathBuf::from(shown));
        assert_eq!(
            to_linux("\u{f02e}\u{f03a}"),
            "\u{f02e}:",
            "a dot is no escape"
        );
        let old = split(Path::new(r"\\WSL$\Debian"));
        assert_eq!(old, Some((r"\\WSL$\Debian", "/".into())));
        assert_eq!(split(Path::new(r"\\wsl.localhost\")), None);
        assert_eq!(split(Path::new(r"\\server\share\x")), None);
        assert_eq!(split(Path::new(r"C:\Users\me")), None);
    }
}
