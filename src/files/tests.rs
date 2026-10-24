use std::env;

use super::*;

/// A fresh folder under the temp dir.
pub(super) fn folder(name: &str) -> PathBuf {
    let folder = env::temp_dir().join(format!("ely-tea-party-{name}-{}", std::process::id()));
    if folder.exists() {
        fs::remove_dir_all(&folder).expect("a stale test folder goes");
    }
    fs::create_dir_all(&folder).expect("a test folder");
    folder
}

#[test]
fn folders_come_first_then_files_by_name_ignoring_case() {
    let root = folder("files-list");
    for name in ["Zeta", "alpha"] {
        fs::create_dir(root.join(name)).expect("a folder");
    }
    for name in ["c.txt", "B.txt", "a.txt"] {
        fs::write(root.join(name), "").expect("a file");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(root.join("Zeta"), root.join("link")).expect("a link to a folder");
        symlink(root.join("nowhere"), root.join("broken")).expect("a broken link");
    }
    let expected: &[(&str, bool)] = if cfg!(unix) {
        &[
            ("alpha", true),
            ("link", true),
            ("Zeta", true),
            ("a.txt", false),
            ("B.txt", false),
            ("broken", false),
            ("c.txt", false),
        ]
    } else {
        &[
            ("alpha", true),
            ("Zeta", true),
            ("a.txt", false),
            ("B.txt", false),
            ("c.txt", false),
        ]
    };
    let listed: Vec<_> = list(&root).expect("a listing");
    let listed: Vec<_> = listed
        .iter()
        .map(|entry| (entry.name.to_str().expect("a UTF-8 name"), entry.folder))
        .collect();
    assert_eq!(listed, expected);
    assert!(list(&root.join("missing")).is_err());
}

#[test]
fn a_mark_dots_each_folder_above_it_up_to_the_root() {
    let root = Path::new("/r");
    let changes = vec![
        (
            root.join("src").join("ui").join("a.rs"),
            GitStatus::Modified,
        ),
        (root.join("gone.txt"), GitStatus::Deleted),
    ];
    let (marks, dirty) = marks(root, changes);
    assert_eq!(marks.len(), 2);
    let expected = HashSet::from([
        root.to_path_buf(),
        root.join("src"),
        root.join("src").join("ui"),
    ]);
    assert_eq!(dirty, expected);
}

#[test]
fn the_tree_wears_marks_and_an_untracked_folder_marks_what_it_holds() {
    let root = PathBuf::from("/r");
    let (notes, src, locked) = (root.join("notes"), root.join("src"), root.join("locked"));
    let entry = |name: &str, folder| Entry {
        name: name.into(),
        folder,
    };
    let mut files = Files::new(root.clone());
    let top = vec![
        entry("notes", true),
        entry("src", true),
        entry("locked", true),
        entry("a.txt", false),
    ];
    files.listings.insert(root.clone(), Ok(top));
    let inside = vec![entry("deep", true), entry("todo.txt", false)];
    files.listings.insert(notes.clone(), Ok(inside));
    files
        .listings
        .insert(src.clone(), Ok(vec![entry("main.rs", false)]));
    files.listings.insert(locked.clone(), Err("denied".into()));
    let changes = vec![
        (notes.clone(), GitStatus::Untracked),
        (src.join("main.rs"), GitStatus::Modified),
    ];
    (files.marks, files.dirty) = marks(&root, changes);

    files.tree.open.extend([notes.clone(), src.clone()]);
    let rows = files.rows();
    let observed: Vec<_> = rows
        .iter()
        .map(|row| {
            (
                row.path.strip_prefix(&root).unwrap().to_path_buf(),
                row.mark,
                row.dirty,
                row.note,
            )
        })
        .collect();
    let expected = vec![
        ("", None, true, None),
        ("notes", Some(GitStatus::Untracked), false, None),
        ("notes/deep", Some(GitStatus::Untracked), false, None),
        ("notes/todo.txt", Some(GitStatus::Untracked), false, None),
        ("src", None, true, None),
        ("src/main.rs", Some(GitStatus::Modified), false, None),
        ("locked", None, false, Some("unreadable")),
        ("a.txt", None, false, None),
    ]
    .into_iter()
    .map(|(path, mark, dirty, note)| (PathBuf::from(path), mark, dirty, note))
    .collect::<Vec<_>>();
    assert_eq!(observed, expected);
    files.tree.open.remove(&notes);
    assert!(
        !files
            .rows()
            .iter()
            .any(|row| row.path == notes.join("todo.txt"))
    );
}

#[test]
fn names_with_newlines_label_on_one_line_and_keep_their_paths() {
    let root = PathBuf::from("/r");
    let (folder, file) = (root.join("line\nbreak"), root.join("report\n2026.txt"));
    let mut files = Files::new(root.clone());
    let entries = vec![
        Entry {
            name: "line\nbreak".into(),
            folder: true,
        },
        Entry {
            name: "report\n2026.txt".into(),
            folder: false,
        },
    ];
    files.listings.insert(root.clone(), Ok(entries));
    let rows = files.rows();
    assert_eq!(rows[1].path, folder);
    assert_eq!(rows[1].label, "line␊break");
    assert_eq!(rows[2].path, file);
    assert_eq!(rows[2].label, "report␊2026.txt");
}

#[cfg(unix)]
#[test]
fn names_that_show_alike_keep_their_own_keys_and_paths() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
    let root = PathBuf::from("/r");
    let raw = OsStr::from_bytes(b"bad\xff.txt");
    let (text, bytes) = (root.join("bad\u{FFFD}.txt"), root.join(raw));
    let mut files = Files::new(root.clone());
    let entry = |name: &OsStr| Entry {
        name: name.into(),
        folder: false,
    };
    let entries = vec![entry(OsStr::new("bad\u{FFFD}.txt")), entry(raw)];
    files.listings.insert(root.clone(), Ok(entries));
    assert_ne!(key(&text), key(&bytes), "two paths, two keys");
    let rows = files.rows();
    assert_eq!(rows[1].label, rows[2].label);
    assert_eq!(rows[1].path, text);
    assert_eq!(rows[2].path, bytes);
    assert_ne!(key(&rows[1].path), key(&rows[2].path));
}

/// Runs git in `root` as a fixed author, unsigned.
#[cfg(target_os = "macos")]
fn run(root: &Path, args: &[&str]) {
    let done = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "user.name=Ely", "-c", "user.email=ely@example.com"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .expect("git runs");
    let error = String::from_utf8_lossy(&done.stderr);
    assert!(done.status.success(), "git {args:?}: {error}");
}

#[cfg(target_os = "macos")]
#[test]
fn decomposed_names_on_disk_wear_the_composed_marks_git_writes() {
    let root = folder("files-nfd");
    run(&root, &["init", "-q"]);
    run(&root, &["config", "core.precomposeunicode", "true"]);
    run(&root, &["config", "status.showUntrackedFiles", "normal"]);
    let (dossier, tracked, untracked) = (
        "dossier-e\u{301}",
        "tracked-e\u{301}.txt",
        "new-e\u{301}.txt",
    );
    fs::create_dir(root.join(dossier)).expect("a folder");
    fs::write(root.join(dossier).join("inner.txt"), "first").expect("a file");
    fs::write(root.join(tracked), "first").expect("a file");
    run(&root, &["add", "."]);
    run(&root, &["commit", "-qm", "first"]);
    fs::write(root.join(dossier).join("inner.txt"), "second").expect("an edit");
    fs::write(root.join(tracked), "second").expect("an edit");
    fs::write(root.join(untracked), "new").expect("an untracked file");

    let mut files = Files::new(root.clone());
    files.listings.insert(root.clone(), list(&root));
    let changes = git::changes(&root).expect("status").expect("a work tree");
    (files.marks, files.dirty) = marks(&root, changes);
    let rows = files.rows();
    for (name, mark, dirty) in [
        (".git", None, false),
        (dossier, None, true),
        (untracked, Some(GitStatus::Untracked), false),
        (tracked, Some(GitStatus::Modified), false),
    ] {
        let row = rows.iter().find(|row| row.path == root.join(name)).unwrap();
        assert_eq!(row.mark, mark, "{name}");
        assert_eq!(row.dirty, dirty, "{name}");
    }
}
