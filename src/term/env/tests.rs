use super::*;

fn env(pairs: &[(&str, &str)]) -> Env {
    let mut env = Env::default();
    for (name, value) in pairs {
        env.set(name, value.to_string());
    }
    env
}

fn values(rows: &[(&str, u32, &str)]) -> Vec<(String, u32, String)> {
    let row =
        |(name, kind, value): &(&str, u32, &str)| (name.to_string(), *kind, value.to_string());
    rows.iter().map(row).collect()
}

#[test]
fn a_name_expands_once_from_what_is_held() {
    let held = env(&[("SCOOP_PATH", "D:\\Scoop\\shims"), ("A", "%B%")]);
    assert_eq!(expand("%scoop_path%\\bin", &held), "D:\\Scoop\\shims\\bin");
    assert_eq!(expand("%A%", &held), "%B%", "once, as Windows expands");
    assert_eq!(expand("%NOPE%\\x", &held), "%NOPE%\\x");
    assert_eq!(expand("50% off", &held), "50% off");
}

#[test]
fn a_name_that_names_itself_grows_no_further_than_once() {
    let looped = "%LOOP%".repeat(64);
    let held = env(&[("LOOP", &looped)]);
    assert_eq!(expand(&looped, &held).len(), looped.len() * 64);
}

#[test]
fn a_name_matches_as_windows_matches_it() {
    let mut held = env(&[("CAFÉ", "C:\\Tools"), ("term", "dumb")]);
    assert_eq!(expand("%café%\\bin", &held), "C:\\Tools\\bin");
    held.set("TERM", "xterm-256color".into());
    assert_eq!(held.0.len(), 2, "one name: {held:?}");
    assert_eq!(held.get("Term"), Some("xterm-256color"));
}

#[test]
fn a_key_sets_plain_text_first_and_joins_path_variables() {
    let mut held = env(&[("SystemRoot", "C:\\old")]);
    let machine = values(&[
        ("Path", REG_EXPAND_SZ, "%SystemRoot%\\system32"),
        ("SystemRoot", REG_SZ, "C:\\WINDOWS"),
        ("Empty", REG_SZ, ""),
    ]);
    apply(&mut held, &machine);
    apply(
        &mut held,
        &values(&[("PATH", REG_SZ, "%SystemRoot%\\user")]),
    );
    assert_eq!(
        held.get("path"),
        Some("C:\\WINDOWS\\system32;C:\\WINDOWS\\user")
    );
    assert_eq!(held.get("Empty"), None);
    assert!(is_path("OS2LIBPATH") && !is_path("PATHEXT"));
}

#[test]
fn a_folder_is_listed_once_and_this_process_s_own_come_after() {
    assert_eq!(dedupe("C:\\a;C:\\b;c:\\A\\", None), "C:\\a;C:\\b");
    let held = Some("C:\\old;C:\\a;C:\\%OLD%");
    assert_eq!(dedupe("C:\\a", held), "C:\\a;C:\\old");
}

#[test]
fn a_path_only_this_process_holds_stays_without_the_registry_s() {
    let held = env(&[("PATH", "C:\\portable;C:\\%OLD%;C:\\portable")]);
    let mut registry = env(&[("LibPath", "C:\\lib;C:\\lib")]);
    settle(&mut registry, &held);
    assert_eq!(registry.get("Path"), Some("C:\\portable"));
    assert_eq!(registry.get("LIBPATH"), Some("C:\\lib"));
}

#[test]
fn a_session_gets_the_registry_path() {
    let path = path().expect("the registry reads");
    let mut seen = HashSet::new();
    for entry in path.split(';').filter(|entry| !entry.is_empty()) {
        let key = entry.trim_end_matches('\\').to_lowercase();
        assert!(seen.insert(key), "{entry} is listed twice");
    }
    assert!(path.to_lowercase().contains("system32"), "{path}");
}
