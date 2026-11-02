use std::path::{Path, PathBuf};

use ely_gpui_component::{
    i18n::text,
    menus::{Menu, MenuItem},
};
use gpui::{App, ClipboardItem, Context, MouseButton, MouseDownEvent, Pixels, Point, Window};

use super::{mouse::Tracking, view::TerminalView};
use crate::files;

/// The menu a right press on a file opens.
#[derive(Default)]
pub struct FileMenu {
    serial: u64,
    /// The press's point, until a render hands it on.
    at: Option<Point<Pixels>>,
    path: Option<PathBuf>,
}

impl FileMenu {
    fn ask(&mut self, path: PathBuf, at: Point<Pixels>) {
        self.serial += 1;
        self.at = Some(at);
        self.path = Some(path);
    }

    /// The menu's request, once: a remounted menu would replay it.
    pub(super) fn request(&mut self) -> Option<(u64, Point<Pixels>)> {
        Some((self.serial, self.at.take()?))
    }

    /// The menu has closed, or is about to.
    pub(super) fn forget(&mut self) {
        self.at = None;
        self.path = None;
    }

    pub(super) fn items(&self, cx: &App) -> Menu {
        self.path
            .as_deref()
            .map_or_else(Menu::new, |path| items(path, cx))
    }
}

fn items(path: &Path, cx: &App) -> Menu {
    let (open, reveal, copy) = (path.to_path_buf(), path.to_path_buf(), path.to_path_buf());
    Menu::new()
        .item(
            MenuItem::new(text(cx, "et.files.open", &[])).on_click(move |_, cx| {
                log::info!("terminal: opening {}", open.display());
                cx.open_with_system(&open);
            }),
        )
        .item(
            MenuItem::new(text(cx, files::REVEAL, &[])).on_click(move |_, cx| {
                log::info!("terminal: revealing {}", reveal.display());
                cx.reveal_path(&reveal);
            }),
        )
        .item(
            MenuItem::new(text(cx, "et.files.copy_path", &[])).on_click(move |_, cx| {
                match copy.to_str() {
                    Some(text) => {
                        log::info!("terminal: copied {text}");
                        cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
                    }
                    None => log::error!("terminal: {} is no UTF-8 to copy", copy.display()),
                }
            }),
        )
}

impl TerminalView {
    /// A press: a right one on a file asks for the menu; one beside the open menu only closes it.
    pub(super) fn press(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // The open menu holds the focus, and gives it back as it closes.
        let open = self.menu.path.is_some() && !self.focus.is_focused(window);
        let (_, point, _) = self.cell_at(event.position);
        let free = self.tracking(&event.modifiers) == Tracking::Off;
        let file = (event.button == MouseButton::Right && free)
            .then(|| self.file_at(point))
            .flatten();
        if let Some(path) = file {
            log::info!("terminal {}: menu for {}", self.pty.pid, path.display());
            if !open {
                window.focus(&self.focus, cx);
            }
            window.prevent_default();
            cx.stop_propagation();
            self.menu.ask(path, event.position);
            cx.notify();
        } else if open {
            log::debug!("terminal {}: a press closes the menu", self.pty.pid);
            self.menu.forget();
            window.focus(&self.focus, cx);
            cx.notify();
        } else {
            self.mouse_down(event, window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui::{point, px};

    use super::*;

    #[test]
    fn a_request_goes_out_once() {
        let mut menu = FileMenu::default();
        assert_eq!(menu.request(), None);
        let at = point(px(3.), px(4.));
        menu.ask(PathBuf::from("/a"), at);
        assert_eq!(menu.request(), Some((1, at)));
        assert_eq!(menu.request(), None, "a remounted menu must not reopen");
        assert_eq!(menu.path.as_deref(), Some(Path::new("/a")), "its rows stay");
        menu.ask(PathBuf::from("/b"), at);
        assert_eq!(menu.request(), Some((2, at)), "a new press asks anew");
        menu.forget();
        assert_eq!(menu.path, None);
    }
}
