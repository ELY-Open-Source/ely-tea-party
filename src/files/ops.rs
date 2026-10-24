use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

use anyhow::{Context, ensure};

#[derive(Debug)]
pub(super) enum Operation {
    Create {
        parent: PathBuf,
        name: String,
        folder: bool,
    },
    Trash(PathBuf),
}

impl Operation {
    pub fn path(&self) -> PathBuf {
        match self {
            Self::Create { parent, name, .. } => parent.join(name),
            Self::Trash(path) => path.clone(),
        }
    }

    pub fn run(&self) -> anyhow::Result<()> {
        match self {
            Self::Create {
                parent,
                name,
                folder,
            } => {
                validate_name(name)?;
                let path = parent.join(name);
                if *folder {
                    fs::create_dir(&path)
                        .with_context(|| format!("cannot create folder {}", path.display()))?;
                } else {
                    OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&path)
                        .with_context(|| format!("cannot create file {}", path.display()))?;
                }
                Ok(())
            }
            Self::Trash(path) => {
                ensure!(
                    path.is_absolute() && path.parent().is_some() && path.file_name().is_some(),
                    "cannot trash a filesystem root"
                );
                trash(path).with_context(|| format!("cannot move {} to Trash", path.display()))
            }
        }
    }
}

pub(super) fn validate_name(name: &str) -> anyhow::Result<()> {
    ensure!(!name.trim().is_empty(), "Enter a name.");
    ensure!(name != "." && name != "..", "Use a file or folder name.");
    ensure!(
        !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\')),
        "The name cannot contain a slash or control character."
    );
    #[cfg(windows)]
    {
        ensure!(
            !name.contains(['<', '>', ':', '"', '|', '?', '*']) && !name.ends_with(['.', ' ']),
            "This name is not supported by Windows."
        );
        let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
        ensure!(
            !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                && !(stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && matches!(stem.as_bytes()[3], b'1'..=b'9')),
            "This name is reserved by Windows."
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn trash(path: &Path) -> anyhow::Result<()> {
    use objc::{
        class, msg_send,
        runtime::{BOOL, NO, Object},
        sel, sel_impl,
    };
    use std::{
        ffi::{CStr, CString},
        os::unix::ffi::OsStrExt,
        ptr,
    };
    let path = CString::new(path.as_os_str().as_bytes())?;
    unsafe {
        let pool: *mut Object = msg_send![class!(NSAutoreleasePool), new];
        let url: *mut Object = msg_send![class!(NSURL), fileURLWithFileSystemRepresentation: path.as_ptr() isDirectory: NO relativeToURL: ptr::null_mut::<Object>()];
        let manager: *mut Object = msg_send![class!(NSFileManager), defaultManager];
        let mut error: *mut Object = ptr::null_mut();
        let done: BOOL = msg_send![manager, trashItemAtURL: url resultingItemURL: ptr::null_mut::<*mut Object>() error: &mut error];
        let result = if done != NO {
            Ok(())
        } else {
            let message = if error.is_null() {
                "The system could not move this item to Trash.".into()
            } else {
                let description: *mut Object = msg_send![error, localizedDescription];
                let text: *const std::ffi::c_char = msg_send![description, UTF8String];
                if text.is_null() {
                    "The system returned an unreadable error.".into()
                } else {
                    CStr::from_ptr(text).to_string_lossy().into_owned()
                }
            };
            Err(anyhow::anyhow!("{message}"))
        };
        let _: () = msg_send![pool, drain];
        result
    }
}

#[cfg(target_os = "linux")]
fn trash(path: &Path) -> anyhow::Result<()> {
    let output = std::process::Command::new("gio")
        .arg("trash")
        .arg("--")
        .arg(path)
        .output()
        .context("cannot run gio trash")?;
    ensure!(
        output.status.success(),
        "gio trash: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[cfg(windows)]
fn trash(path: &Path) -> anyhow::Result<()> {
    let script = "$ErrorActionPreference='Stop'; Add-Type -AssemblyName Microsoft.VisualBasic; $p=$env:ELY_TRASH_PATH; if ([System.IO.Directory]::Exists($p)) { [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory($p, 'OnlyErrorDialogs', 'SendToRecycleBin', 'ThrowException') } else { [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile($p, 'OnlyErrorDialogs', 'SendToRecycleBin', 'ThrowException') }";
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("ELY_TRASH_PATH", path)
        .output()
        .context("cannot open the Windows Recycle Bin")?;
    ensure!(
        output.status.success(),
        "Recycle Bin: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn trash(_: &Path) -> anyhow::Result<()> {
    anyhow::bail!("This platform has no Trash integration.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trash_refuses_roots_and_relative_targets() {
        for path in [
            PathBuf::from(std::path::MAIN_SEPARATOR_STR),
            PathBuf::from("relative.txt"),
        ] {
            assert!(Operation::Trash(path).run().is_err());
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_trash_reports_a_missing_file() {
        let root = super::super::tests::folder("files-trash-missing");
        let result = Operation::Trash(root.join("missing 文件.txt")).run();
        assert!(result.is_err());
        assert!(root.is_dir());
    }

    #[test]
    fn creation_rejects_paths_and_never_overwrites_an_entry() {
        let root = super::super::tests::folder("files-create");
        for name in [
            "",
            " ",
            ".",
            "..",
            "../escape",
            "/absolute",
            "a/b",
            "a\\b",
            "bad\0name",
            "two\nlines",
        ] {
            assert!(validate_name(name).is_err(), "{name:?}");
        }
        let create = |name: &str, folder| {
            Operation::Create {
                parent: root.clone(),
                name: name.into(),
                folder,
            }
            .run()
        };
        create("文件 with spaces.txt", false).unwrap();
        fs::write(root.join("文件 with spaces.txt"), "keep me").unwrap();
        assert!(create("文件 with spaces.txt", false).is_err());
        assert_eq!(
            fs::read_to_string(root.join("文件 with spaces.txt")).unwrap(),
            "keep me"
        );
        create(".hidden folder", true).unwrap();
        assert!(create(".hidden folder", false).is_err());
        assert!(create(".hidden folder", true).is_err());
        assert!(create("文件 with spaces.txt", true).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn creation_does_not_follow_an_existing_symlink() {
        let root = super::super::tests::folder("files-create-link");
        let target = root.join("target");
        fs::write(&target, "keep me").unwrap();
        std::os::unix::fs::symlink(&target, root.join("link")).unwrap();
        let operation = Operation::Create {
            parent: root.clone(),
            name: "link".into(),
            folder: false,
        };
        assert!(operation.run().is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), "keep me");
    }
}
