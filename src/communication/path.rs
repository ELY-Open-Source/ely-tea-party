use std::{env, ffi::OsStr, path::Path};

use anyhow::Context;

pub fn prepend(directory: &Path, inherited: &OsStr) -> anyhow::Result<String> {
    let paths = std::iter::once(directory.to_path_buf())
        .chain(env::split_paths(inherited).filter(|path| path != directory));
    env::join_paths(paths)?
        .into_string()
        .map_err(|_| anyhow::anyhow!("CLI: PATH is not UTF-8"))
}

pub fn environment() -> anyhow::Result<Vec<(String, String)>> {
    let executable = env::current_exe().context("CLI: cannot locate the running app")?;
    let directory = executable
        .parent()
        .context("CLI: executable has no directory")?;
    #[cfg(unix)]
    let inherited = env::var_os("PATH").unwrap_or_default();
    #[cfg(windows)]
    let inherited = std::ffi::OsString::from(crate::term::env::path()?);
    Ok(vec![
        ("ELY_CLI".into(), executable.to_string_lossy().into_owned()),
        (
            "ELY_BIN_DIR".into(),
            directory.to_string_lossy().into_owned(),
        ),
        ("PATH".into(), prepend(directory, &inherited)?),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_directory_is_first_once_and_spaces_are_preserved() {
        let app = Path::new("/Applications/Ely Tea Party.app/Contents/MacOS");
        let paths = env::join_paths([Path::new("/bin"), app, Path::new("/usr/bin"), app]).unwrap();
        let value = prepend(app, &paths).unwrap();
        assert_eq!(
            env::split_paths(&value).collect::<Vec<_>>(),
            [app, Path::new("/bin"), Path::new("/usr/bin")]
        );
        assert_eq!(prepend(app, OsStr::new(&value)).unwrap(), value);
    }
}
