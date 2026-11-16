use super::*;

#[test]
fn under_wsl_each_shell_reports_its_folder_as_windows_names_it() {
    // wslpath's stand-in names a folder under PREFIX: the share `\\wsl`, which Unix reads as `/wsl`, or a drive.
    let fake = folder("wslpath");
    let names = r#"printf '%s%s\n' "$PREFIX" "$(printf %s "$2" | tr / '\\')""#;
    fs::write(fake.join("wslpath"), format!("#!/bin/sh\n{names}\n")).expect("wslpath");
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(fake.join("wslpath"), fs::Permissions::from_mode(0o755)).expect("a mode");
    let path = format!("{}:{}", fake.display(), env::var("PATH").expect("PATH"));
    for (prefix, shell) in ["zsh", "bash", "fish"]
        .iter()
        .flat_map(|shell| [("\\\\wsl", shell), ("C:", shell)])
    {
        let wsl = [("ELY_WSL", "1"), ("PATH", &path), ("PREFIX", prefix)];
        let mut run = Run::with(
            &installed(shell),
            &[],
            wsl.map(|(k, v)| (k.into(), v.into())).to_vec(),
        );
        // fish reports its own folder too, which Windows passes by as naming none of its own.
        let unix = if prefix == "C:" { "/C:" } else { "/wsl" };
        let start = Path::new(unix).join(run.start.strip_prefix("/").expect("a full path"));
        run.until("the folder", |run| run.folders.contains(&start));
        run.send("cd in*");
        run.until("inner folder", |run| {
            run.folders.contains(&start.join(INNER))
        });
        run.finish();
    }
    fs::remove_dir_all(&fake).expect("cleanup");
}
