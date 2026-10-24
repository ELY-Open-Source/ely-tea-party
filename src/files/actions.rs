use std::path::PathBuf;

use ely_gpui_component::{
    forms::TextInput,
    menus::{Menu, MenuItem},
    overlays::{AlertDialog, PromptDialog},
    primitives::Severity,
};
use gpui::{AppContext, Context, Entity, Subscription, Window};

use super::{Files, ops};
use crate::sessions;

pub(super) struct Creation {
    parent: PathBuf,
    folder: bool,
    input: Entity<TextInput>,
    _changes: Subscription,
}

impl Files {
    pub(super) fn refresh(&mut self, cx: &mut Context<Self>) {
        log::info!("files: refreshing {}", self.root.display());
        self.show(&self.root.clone(), cx);
    }

    pub(super) fn menu(&self, path: PathBuf, folder: bool, cx: &Context<Self>) -> Menu {
        let refresh = cx.entity();
        let mut menu = Menu::new().item(MenuItem::new("Refresh").on_click(move |_, cx| {
            refresh.update(cx, |files, cx| files.refresh(cx));
        }));
        let parent = if folder {
            Some(path.clone())
        } else {
            path.parent().map(|p| p.to_path_buf())
        };
        for (label, folder) in [("New File", false), ("New Folder", true)] {
            let (files, parent) = (cx.entity(), parent.clone());
            menu = menu.item(
                MenuItem::new(label)
                    .disabled(self.busy || parent.is_none())
                    .on_click(move |window, cx| {
                        if let Some(parent) = &parent {
                            files.update(cx, |files, cx| {
                                files.begin_create(parent.clone(), folder, window, cx)
                            });
                        }
                    }),
            );
        }
        let reveal = path.clone();
        let label = if cfg!(target_os = "macos") {
            "Open in Finder"
        } else {
            "Open in File Manager"
        };
        menu = menu
            .separator()
            .item(MenuItem::new(label).on_click(move |_, cx| {
                log::info!("files: revealing {}", reveal.display());
                cx.reveal_path(&reveal);
            }));
        let files = cx.entity();
        menu.separator().item(
            MenuItem::new("Move to Trash")
                .disabled(self.busy || path == self.root)
                .on_click(move |_, cx| {
                    files.update(cx, |files, cx| {
                        files.run(ops::Operation::Trash(path.clone()), cx)
                    });
                }),
        )
    }

    fn begin_create(
        &mut self,
        parent: PathBuf,
        folder: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        let input = cx.new(|cx| TextInput::new(window, cx));
        let changes = cx.observe(&input, |_, _, cx| cx.notify());
        self.creating = Some(Creation {
            parent,
            folder,
            input,
            _changes: changes,
        });
        cx.notify();
    }

    pub(super) fn creation_dialog(&self, cx: &Context<Self>) -> Option<PromptDialog> {
        let creating = self.creating.as_ref()?;
        let (close, submit) = (cx.entity(), cx.entity());
        let (parent, folder) = (creating.parent.clone(), creating.folder);
        Some(
            PromptDialog::new(
                "files-create",
                if folder { "New Folder" } else { "New File" },
                &creating.input,
                move |_, cx| {
                    close.update(cx, |files, cx| {
                        files.creating = None;
                        cx.notify();
                    });
                },
            )
            .label(format!(
                "Name in {}",
                sessions::shown(&parent, dirs::home_dir().as_deref())
            ))
            .submit("Create")
            .check(|name| ops::validate_name(name).map_err(|error| error.to_string().into()))
            .on_submit(move |name, _, cx| {
                let operation = ops::Operation::Create {
                    parent: parent.clone(),
                    name: name.to_string(),
                    folder,
                };
                submit.update(cx, |files, cx| files.run(operation, cx));
            }),
        )
    }

    pub(super) fn error_dialog(&self, cx: &Context<Self>) -> Option<AlertDialog> {
        let error = self.error.clone()?;
        let close = cx.entity();
        Some(AlertDialog::new(
            "files-error",
            Severity::Danger,
            "File operation failed",
            error,
            move |_, cx| {
                close.update(cx, |files, cx| {
                    files.error = None;
                    cx.notify();
                });
            },
        ))
    }

    fn run(&mut self, operation: ops::Operation, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let path = operation.path();
        if matches!(operation, ops::Operation::Trash(_)) && path == self.root {
            return;
        }
        self.busy = true;
        self.error = None;
        let (root, generation) = (self.root.clone(), self.generation);
        log::info!("files: starting {operation:?}");
        let task = cx.background_spawn(async move {
            let result = operation.run();
            (operation, result)
        });
        cx.spawn(async move |files, cx| {
            let (operation, result) = task.await;
            if let Err(error) = &result {
                log::error!("files: {operation:?} failed: {error:#}");
            }
            files
                .update(cx, |files, cx| {
                    if files.root != root || files.generation != generation {
                        return;
                    }
                    files.busy = false;
                    match result {
                        Ok(()) => {
                            log::info!("files: completed {operation:?}");
                            match operation {
                                ops::Operation::Create { parent, .. } => {
                                    files.tree.open.insert(parent.clone());
                                    files.wanted.insert(parent);
                                    files.tree.selected = Some(path);
                                }
                                ops::Operation::Trash(_) => {
                                    files.wanted.retain(|p| !p.starts_with(&path));
                                    files.listings.retain(|p, _| !p.starts_with(&path));
                                    files.tree.open.retain(|p| !p.starts_with(&path));
                                    files.tree.selected = path.parent().map(|p| p.to_path_buf());
                                }
                            }
                            files.tree.menu = None;
                            files.refresh(cx);
                        }
                        Err(error) => {
                            files.error = Some(format!(
                                "{}: {error:#}",
                                sessions::one_line(&path.to_string_lossy())
                            ))
                        }
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
        cx.notify();
    }
}
