use super::*;

/// A throwaway key, never the classes Explorer reads.
fn scratch(name: &str) -> String {
    format!(r"Software\ElyTeaPartyTest\{}-{name}", std::process::id())
}

#[test]
fn the_menu_is_added_read_and_removed_under_its_classes() {
    let classes = scratch("menu");
    let exe = Path::new(r"C:\Program Files\ElyTeaParty\ely-tea-party.exe");
    assert_eq!(read(&classes, exe).expect("a read"), Entry::Absent);
    add(&classes, exe, "Open in Ely Tea Party").expect("added");
    assert_eq!(read(&classes, exe).expect("a read"), Entry::Ours);
    let label = registry::text(&format!(r"{classes}\{}", ENTRIES[1]), None).expect("a read");
    assert_eq!(label, Some("Open in Ely Tea Party".into()));
    let moved = Path::new(r"D:\Tools\ely-tea-party.exe");
    assert_eq!(
        read(&classes, moved).expect("a read"),
        Entry::Other(r#""C:\Program Files\ElyTeaParty\ely-tea-party.exe" "%V""#.into())
    );
    remove(&classes).expect("removed");
    assert_eq!(read(&classes, exe).expect("a read"), Entry::Absent);
    remove(&classes).expect("removing nothing is fine");
    registry::delete_tree(&classes).expect("the scratch key goes");
}
