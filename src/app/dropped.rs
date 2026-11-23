//! Folders the system hands over, as macOS does one dropped on the app's icon.

use anyhow::Context as _;
use gpui::App;

use crate::workspace::Workspace;

/// The folder a `file://` URL the system hands over names, as macOS does a folder dropped on the icon.
pub fn dropped_folder(url: &str) -> anyhow::Result<std::path::PathBuf> {
    crate::term::file_uri(url.as_bytes())
        .filter(|path| path.is_dir())
        .with_context(|| format!("app: {url} names no folder to open"))
}

/// A folder dropped on the app's icon while it runs: a session there in the main window.
pub fn dropped(url: &str, cx: &mut App) -> anyhow::Result<()> {
    let folder = dropped_folder(url)?;
    log::info!("app: {} dropped on the icon", folder.display());
    let main = cx
        .windows()
        .into_iter()
        .find_map(|window| window.downcast::<Workspace>())
        .context("app: no main window for a dropped folder")?;
    main.update(cx, |workspace, window, cx| {
        window.activate_window();
        workspace.here(folder, window, cx)
    })??;
    cx.activate(true);
    Ok(())
}
