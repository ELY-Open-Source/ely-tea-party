use std::{env, fs};

use ely_gpui_component::git::{LineKind, Stretch};
use gpui::SharedString;

use super::*;

/// A fresh folder under the temp dir.
fn folder(name: &str) -> PathBuf {
    let folder = env::temp_dir().join(format!("ely-tea-party-{name}-{}", std::process::id()));
    if folder.exists() {
        fs::remove_dir_all(&folder).expect("a stale test folder goes");
    }
    fs::create_dir_all(&folder).expect("a test folder");
    folder
}

/// Runs git in `root` to set a test up.
fn prepare(root: &Path, args: &[&str]) {
    let done = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git runs");
    let error = String::from_utf8_lossy(&done.stderr);
    assert!(done.status.success(), "git {args:?}: {error}");
}

/// A new repository on `main` whose own config keeps the user's away.
fn repository(name: &str) -> PathBuf {
    let root = folder(name);
    prepare(&root, &["init", "-q", "-b", "main"]);
    for [key, value] in [
        ["user.name", "Ely"],
        ["user.email", "ely@example.com"],
        ["commit.gpgsign", "false"],
        ["core.hooksPath", "no-hooks"],
        ["core.autocrlf", "false"],
        ["merge.conflictStyle", "merge"],
        ["status.showUntrackedFiles", "normal"],
    ] {
        prepare(&root, &["config", key, value]);
    }
    root
}

/// Writes `text` to `path` under `root`, making its folders.
fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().expect("a parent")).expect("folders");
    fs::write(path, text).expect("a file");
}

fn change(path: &str, status: GitStatus) -> Change {
    Change {
        path: path.into(),
        from: None,
        status,
    }
}

fn read(root: &Path) -> Repo {
    repo(root).expect("status").expect("a work tree")
}

#[test]
fn status_records_read_with_head() {
    let out = b"# branch.oid 1a2b\0# branch.head main\0# branch.upstream origin/main\0\
        # branch.ab +1 -0\0\
        1 .M N... 100644 100644 100644 aaa bbb src/a b.rs\0\
        2 RM N... 100644 100644 100644 aaa bbb R100 new name.rs\0old name.rs\0\
        u UU N... 100644 100644 100644 100644 aaa bbb ccc clash.rs\0? notes/\0";
    let (head, records) = parse(out).expect("records");
    let main = Head {
        branch: Some("main".into()),
        commit: Some("1a2b".into()),
    };
    assert_eq!(head, main);
    let record = |x, y, path: &str, from: Option<&str>| Record {
        x,
        y,
        path: path.into(),
        from: from.map(Into::into),
    };
    let expected = [
        record(b'.', b'M', "src/a b.rs", None),
        record(b'R', b'M', "new name.rs", Some("old name.rs")),
        record(b'U', b'U', "clash.rs", None),
        record(b'?', b'?', "notes/", None),
    ];
    assert_eq!(records, expected);
    let unborn = b"# branch.oid (initial)\0# branch.head (detached)\0";
    let (head, _) = parse(unborn).expect("headers");
    assert_eq!(head, Head::default(), "no commit yet, no branch");
}

#[test]
fn records_git_never_writes_are_errors() {
    for out in [
        &b"1 .M N... 100644\0"[..],
        b"x junk\0",
        b"2 R. N... 1 2 3 a b R100 new\0",
        b"1 M N... 1 2 3 a b p\0",
        b"?\0",
        b"1 .M N... 1 2 3 a b \0",
    ] {
        assert!(parse(out).is_err(), "{:?}", String::from_utf8_lossy(out));
    }
}

#[test]
fn both_codes_combine_into_one_mark() {
    use GitStatus::*;
    for (codes, status) in [
        (b"??", Untracked),
        (b"UU", Conflicted),
        (b"AA", Conflicted),
        (b"DU", Conflicted),
        (b".D", Deleted),
        (b"RM", Renamed),
        (b"AM", Added),
        (b"C.", Added),
        (b".T", Modified),
        (b"MM", Modified),
    ] {
        let named = String::from_utf8_lossy(codes);
        assert_eq!(kind(codes[0], codes[1]), Some(status), "{named}");
    }
    assert_eq!(kind(b'.', b'.'), None);
    assert_eq!(kind(b'!', b'!'), None);
}

#[test]
fn a_work_tree_reports_changes_under_the_folder() {
    let root = repository("git-changes");
    for (path, text) in [
        ("kept.txt", "kept"),
        ("gone.txt", "gone"),
        ("src/main.rs", "fn main() {}"),
        ("src/inner/deep.rs", "deep"),
    ] {
        write(&root, path, text);
    }
    prepare(&root, &["add", "."]);
    prepare(&root, &["commit", "-qm", "first"]);
    write(&root, "src/main.rs", "fn main() { }");
    fs::remove_file(root.join("gone.txt")).expect("a removal");
    write(&root, "added.txt", "new");
    prepare(&root, &["add", "added.txt"]);
    write(&root, "notes/sub/todo.txt", "todo");

    let mut all = changes(&root).expect("status").expect("a work tree");
    all.sort_by(|a, b| a.0.cmp(&b.0));
    let expected = [
        ("added.txt", GitStatus::Added),
        ("gone.txt", GitStatus::Deleted),
        ("notes", GitStatus::Untracked),
        ("src/main.rs", GitStatus::Modified),
    ]
    .map(|(path, status)| (root.join(path), status));
    assert_eq!(all, expected);
    let src = root.join("src");
    let main = (src.join("main.rs"), GitStatus::Modified);
    assert_eq!(changes(&src).expect("status"), Some(vec![main]));
    let sub = root.join("notes").join("sub");
    let whole = (sub.clone(), GitStatus::Untracked);
    assert_eq!(
        changes(&sub).expect("status"),
        Some(vec![whole]),
        "a folder inside an untracked one is untracked whole"
    );
    assert_eq!(changes(&root.join(".git")).expect("rev-parse"), None);
    assert_eq!(changes(&folder("git-outside")).expect("rev-parse"), None);
}

#[cfg(unix)]
#[test]
fn a_folder_named_with_a_newline_keeps_its_whole_prefix() {
    let root = repository("git-newline");
    write(&root, "line\nbreak/tracked.txt", "first");
    prepare(&root, &["add", "."]);
    prepare(&root, &["commit", "-qm", "first"]);
    write(&root, "line\nbreak/tracked.txt", "second");
    let inner = root.join("line\nbreak");
    let edit = (inner.join("tracked.txt"), GitStatus::Modified);
    assert_eq!(changes(&inner).expect("status"), Some(vec![edit]));
}

#[test]
fn a_repository_lists_its_branch_and_each_section() {
    let root = repository("git-repo");
    for path in ["kept.txt", "gone.txt", "old.txt", "src/main.rs"] {
        write(&root, path, "first\n");
    }
    prepare(&root, &["add", "."]);
    prepare(&root, &["commit", "-qm", "first"]);
    write(&root, "src/main.rs", "second\n");
    fs::remove_file(root.join("gone.txt")).expect("a removal");
    write(&root, "added.txt", "new\n");
    write(&root, "kept.txt", "staged\n");
    prepare(&root, &["add", "added.txt", "kept.txt"]);
    write(&root, "kept.txt", "staged, then changed\n");
    prepare(&root, &["mv", "old.txt", "new.txt"]);
    write(&root, "notes/todo.txt", "todo\n");

    let state = read(&root.join("src"));
    let real = |path: &Path| fs::canonicalize(path).expect("a real path");
    assert_eq!(
        real(&state.top),
        real(&root),
        "the top, from a folder under it"
    );
    assert_eq!(state.branch.as_deref(), Some("main"));
    assert!(state.commit.is_some());
    let renamed = Change {
        path: "new.txt".into(),
        from: Some("old.txt".into()),
        status: GitStatus::Renamed,
    };
    let staged = [
        change("added.txt", GitStatus::Added),
        change("kept.txt", GitStatus::Modified),
        renamed,
    ];
    assert_eq!(state.staged, staged);
    let unstaged = [
        change("gone.txt", GitStatus::Deleted),
        change("kept.txt", GitStatus::Modified),
        change("src/main.rs", GitStatus::Modified),
    ];
    assert_eq!(state.unstaged, unstaged);
    assert_eq!(state.untracked, [change("notes/", GitStatus::Untracked)]);
    assert!(state.conflicted.is_empty());

    prepare(&root, &["commit", "-qm", "second"]);
    prepare(&root, &["checkout", "-q", "--detach"]);
    assert_eq!(read(&root).branch, None, "a detached HEAD has no branch");
    assert_eq!(repo(&root.join(".git")).expect("rev-parse"), None);
    assert_eq!(repo(&folder("git-repo-outside")).expect("rev-parse"), None);
}

#[test]
fn files_stage_and_unstage_by_their_literal_paths() {
    let root = repository("git-stage");
    write(&root, "star[1].txt", "a\n");
    write(&root, "star1.txt", "b\n");
    let star = change("star[1].txt", GitStatus::Untracked);
    stage(&root, &[star]).expect("a stage before the first commit");
    let state = read(&root);
    assert_eq!(state.commit, None);
    let added = change("star[1].txt", GitStatus::Added);
    assert_eq!(state.staged, [added], "the brackets match nothing else");
    assert_eq!(state.untracked, [change("star1.txt", GitStatus::Untracked)]);
    unstage(&root, &state.staged).expect("an unstage before the first commit");
    let state = read(&root);
    assert!(state.staged.is_empty(), "{:?}", state.staged);
    assert_eq!(state.untracked.len(), 2);

    stage(&root, &state.untracked).expect("a stage");
    commit(&root, "first", false).expect("a commit");
    prepare(&root, &["mv", "star1.txt", "moved.txt"]);
    unstage(&root, &read(&root).staged).expect("an unstaged rename");
    let state = read(&root);
    assert!(
        state.staged.is_empty(),
        "both sides of the rename: {state:?}"
    );
    assert_eq!(state.unstaged, [change("star1.txt", GitStatus::Deleted)]);
    assert_eq!(state.untracked, [change("moved.txt", GitStatus::Untracked)]);
    assert!(
        stage(&root, &[]).is_err(),
        "no path would stage the whole tree"
    );
    assert!(unstage(&root, &[]).is_err());
}

#[cfg(unix)]
#[test]
fn a_name_with_a_newline_stages_whole() {
    let root = repository("git-stage-newline");
    write(&root, "line\nfeed.txt", "a\n");
    let new = change("line\nfeed.txt", GitStatus::Untracked);
    stage(&root, &[new]).expect("a stage");
    let added = change("line\nfeed.txt", GitStatus::Added);
    assert_eq!(read(&root).staged, [added]);
}

#[test]
fn a_commit_takes_the_staged_files_and_heads_the_log() {
    let root = repository("git-commit");
    write(&root, "a.txt", "a\n");
    stage(&root, &[change("a.txt", GitStatus::Untracked)]).expect("a stage");
    commit(&root, "first subject\n\nIts body.", false).expect("a commit");
    write(&root, "b.txt", "b\n");
    stage(&root, &[change("b.txt", GitStatus::Untracked)]).expect("a stage");
    commit(&root, "second", false).expect("a commit");

    let commits = log(&root, 10).expect("a log");
    let [newest, first] = commits.as_slice() else {
        panic!("two commits: {commits:?}");
    };
    assert_eq!(newest.subject.as_ref(), "second");
    assert_eq!(first.subject.as_ref(), "first subject");
    assert_eq!(newest.parents, std::slice::from_ref(&first.id));
    assert!(first.parents.is_empty());
    assert!(newest.id.chars().all(|c| c.is_ascii_hexdigit()) && !newest.id.is_empty());
    assert_eq!(newest.author.as_ref(), "Ely");
    assert!(newest.when.ends_with("ago"), "{}", newest.when);
    assert_eq!(newest.refs, [SharedString::from("HEAD -> main")]);
    assert!(first.refs.is_empty());
    assert_eq!(log(&root, 1).expect("a log").len(), 1);
    let error = commit(&root, "nothing", false).expect_err("nothing is staged");
    assert!(format!("{error:#}").contains("nothing"), "{error:#}");
}

#[test]
fn a_copy_keeps_no_source_to_unstage_or_diff() {
    let root = repository("git-copy");
    prepare(&root, &["config", "status.renames", "copies"]);
    prepare(&root, &["config", "diff.renames", "copies"]);
    let text: String = (1..=50).map(|n| format!("line {n}\n")).collect();
    write(&root, "source.txt", &text);
    prepare(&root, &["add", "."]);
    prepare(&root, &["commit", "-qm", "first"]);
    write(&root, "copy.txt", &text);
    write(&root, "source.txt", &format!("{text}more\n"));
    prepare(&root, &["add", "."]);

    let source = change("source.txt", GitStatus::Modified);
    let copy = change("copy.txt", GitStatus::Added);
    assert_eq!(read(&root).staged, [copy.clone(), source.clone()]);
    let stretches = diff(&root, &copy, ChangeSection::Staged).expect("a diff");
    let Some([Stretch::Hunk { lines, .. }]) = stretches.as_deref() else {
        panic!("one hunk: {stretches:?}");
    };
    let added = lines.iter().all(|line| line.kind == LineKind::Added);
    assert!(
        added && lines.len() == 50,
        "the copy's lines alone: {lines:?}"
    );
    unstage(&root, &[copy]).expect("an unstaged copy");
    assert_eq!(read(&root).staged, [source], "the source stays staged");
}

#[cfg(unix)]
#[test]
fn names_that_show_alike_stay_apart() {
    let root = repository("git-bytes");
    write(&root, "bad\u{FFFD}.txt", "text\n");
    prepare(&root, &["add", "."]);
    let blob = run(&root, &["hash-object", "-w", "--stdin"], b"text\n").expect("a blob");
    // A name not in UTF-8, which macOS refuses on disk, goes straight into the index.
    let mut entry = b"100644 ".to_vec();
    entry.extend(blob.trim_ascii());
    entry.extend(b"\tbad\xff.txt\0");
    run(&root, &["update-index", "-z", "--index-info"], &entry).expect("an index entry");

    let raw = <OsStr as std::os::unix::ffi::OsStrExt>::from_bytes(b"bad\xff.txt");
    let staged = read(&root).staged;
    let shown: Vec<_> = staged
        .iter()
        .map(|change| change.path.to_string_lossy())
        .collect();
    assert_eq!(
        shown,
        ["bad\u{FFFD}.txt", "bad\u{FFFD}.txt"],
        "two names show alike"
    );
    let bytes = staged.iter().find(|change| change.path == raw);
    let bytes = bytes.expect("the name in bytes").clone();
    unstage(&root, &[bytes]).expect("an unstage");
    let left = [change("bad\u{FFFD}.txt", GitStatus::Added)];
    assert_eq!(read(&root).staged, left, "the other name stays staged");
}

#[test]
fn an_amend_rewrites_the_last_commit() {
    let root = repository("git-amend");
    assert!(
        commit(&root, "nothing yet", true).is_err(),
        "no commit to amend"
    );
    write(&root, "a.txt", "a\n");
    stage(&root, &[change("a.txt", GitStatus::Untracked)]).expect("a stage");
    commit(&root, "first", false).expect("a commit");
    commit(&root, "first, reworded", true).expect("an amend with nothing staged");
    let commits = log(&root, 10).expect("a log");
    let subjects: Vec<&str> = commits
        .iter()
        .map(|commit| commit.subject.as_ref())
        .collect();
    assert_eq!(subjects, ["first, reworded"]);
}

#[test]
fn a_discard_restores_edits_and_deletes_new_files() {
    let root = repository("git-discard");
    for path in ["edited.txt", "both.txt", "gone.txt"] {
        write(&root, path, "base\n");
    }
    prepare(&root, &["add", "."]);
    prepare(&root, &["commit", "-qm", "base"]);
    write(&root, "edited.txt", "edit\n");
    write(&root, "both.txt", "staged\n");
    prepare(&root, &["add", "both.txt"]);
    write(&root, "both.txt", "staged, then edited\n");
    fs::remove_file(root.join("gone.txt")).expect("a removal");
    write(&root, "intended.txt", "intended\n");
    prepare(&root, &["add", "-N", "intended.txt"]);
    write(&root, "loose.txt", "loose\n");
    write(&root, "notes/todo.txt", "todo\n");
    write(&root, "nested/inner.txt", "inner\n");
    prepare(&root.join("nested"), &["init", "-q"]);

    let state = read(&root);
    for change in &state.unstaged {
        discard(&root, change, ChangeSection::Unstaged).expect("a discarded edit");
    }
    let (nested, loose): (Vec<_>, Vec<_>) =
        (state.untracked.iter()).partition(|change| change.path == Path::new("nested/"));
    for change in loose {
        discard(&root, change, ChangeSection::Untracked).expect("a deleted file");
    }
    let error = discard(&root, nested[0], ChangeSection::Untracked).expect_err("git skips it");
    assert!(
        format!("{error:#}").contains("removed nothing"),
        "{error:#}"
    );
    let staged = read(&root).staged;
    let kept = discard(&root, &staged[0], ChangeSection::Staged);
    assert!(kept.is_err(), "a staged change is not discarded");

    let text = |path| fs::read_to_string(root.join(path)).expect("a file");
    assert_eq!(text("edited.txt"), "base\n");
    assert_eq!(text("both.txt"), "staged\n", "the staged part stays");
    assert_eq!(text("gone.txt"), "base\n", "a deletion is undone");
    for path in ["intended.txt", "loose.txt", "notes"] {
        assert!(!root.join(path).exists(), "{path} goes");
    }
    let state = read(&root);
    assert_eq!(state.staged, [change("both.txt", GitStatus::Modified)]);
    assert!(state.unstaged.is_empty(), "{:?}", state.unstaged);
    assert_eq!(state.untracked, [change("nested/", GitStatus::Untracked)]);
}

mod diffs;
