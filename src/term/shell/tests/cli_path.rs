use std::os::unix::fs::PermissionsExt;

use super::*;

fn check(shell: &str, files: &[(&str, &str)], command: &str) {
    let cli = folder(&format!("cli-path-{}", uuid::Uuid::new_v4())).join("App with spaces");
    fs::create_dir(&cli).unwrap();
    let binary = cli.join("ely-tea-party");
    fs::write(
        &binary,
        "#!/bin/sh\nprintf '%s' ready > \"$HOME/cli-ran\"\n",
    )
    .unwrap();
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
    let mut run = Run::with(
        shell,
        files,
        vec![("ELY_BIN_DIR".into(), cli.to_string_lossy().into_owned())],
    );
    run.until("first prompt", |run| run.marks.contains(&Mark::Input));
    run.send(command);
    let output = run.root.join("home/cli-ran");
    run.until("CLI on PATH after startup configuration", |_| {
        fs::read_to_string(&output).is_ok_and(|text| text == "ready")
    });
    assert_eq!(fs::read_to_string(&output).unwrap(), "ready");
    run.finish();
    fs::remove_dir_all(cli.parent().unwrap()).unwrap();
}

#[test]
fn zsh_restores_app_path_after_zshrc_overwrites_it() {
    check(
        "/bin/zsh",
        &[(".zshrc", "PATH=/usr/bin:/bin\n")],
        "ely-tea-party",
    );
}

#[test]
fn bash_restores_app_path_after_bashrc_overwrites_it() {
    check(
        "/bin/bash",
        &[
            (".bashrc", "PATH=/usr/bin:/bin\n"),
            (".bash_profile", "PATH=/usr/bin:/bin\n"),
        ],
        "ely-tea-party",
    );
}

#[test]
fn fish_restores_app_path_after_config_overwrites_it() {
    let fish = installed("fish");
    check(
        &fish,
        &[(".config/fish/config.fish", "set -gx PATH /usr/bin /bin\n")],
        "ely-tea-party",
    );
}
