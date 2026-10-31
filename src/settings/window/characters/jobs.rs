use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use ely_gpui_component::i18n::text;
use gpui::{App, BorrowAppContext, PathPromptOptions};

use crate::skins::{self, Layout, Library, Meta, Skin, Slot, files};

#[derive(Clone)]
pub(super) enum Action {
    Image,
    Pose(String),
    Import,
    Export(Meta),
    Reset,
    Layout(Layout),
}

enum Done {
    /// The skin applied, and the actions its pack held that were skipped.
    Applied(Arc<Skin>, Vec<String>),
    Reset,
    /// The credits went into the custom skin too, when there is one.
    Exported(Option<Arc<Skin>>),
}

pub(super) fn start(slot: Slot, action: Action, cx: &mut App) {
    if cx.global::<Library>().busy {
        return;
    }
    let active = cx.global::<Library>().active.clone();
    cx.update_global::<Library, _>(|library, _| {
        library.busy = true;
        library.error = None;
        library.notice = None;
        library.skipped.clear();
    });
    log::info!("characters: operation started for {}", slot.id());
    let pick = match action {
        Action::Image | Action::Pose(_) | Action::Import => {
            Some(cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some(text(
                    cx,
                    if matches!(action, Action::Import) {
                        "et.characters.import"
                    } else {
                        "et.characters.image"
                    },
                    &[],
                )),
            }))
        }
        _ => None,
    };
    let export = matches!(action, Action::Export(_)).then(|| {
        let home = dirs::home_dir().unwrap_or_else(std::env::temp_dir);
        cx.prompt_for_new_path(&home, Some(&format!("{}.elyskin", slot.id())))
    });
    cx.spawn(async move |cx| {
        let chosen = async {
            if let Some(pick) = pick {
                Ok(pick.await??.and_then(|paths| paths.into_iter().next()))
            } else if let Some(export) = export {
                Ok(export.await??)
            } else {
                anyhow::Ok(None)
            }
        }
        .await;
        let chosen = match chosen {
            Ok(None)
                if matches!(
                    action,
                    Action::Image | Action::Pose(_) | Action::Import | Action::Export(_)
                ) =>
            {
                cx.update(|cx| cx.update_global::<Library, _>(|library, _| library.busy = false));
                return;
            }
            Ok(path) => path,
            Err(error) => {
                cx.update(|cx| finish(slot, Err(error), cx));
                return;
            }
        };
        let result = cx
            .background_executor()
            .spawn(async move { execute(&skins::folder()?, slot, action, chosen, active) })
            .await;
        cx.update(|cx| finish(slot, result, cx));
    })
    .detach();
}

fn execute(
    folder: &Path,
    slot: Slot,
    action: Action,
    chosen: Option<PathBuf>,
    active: BTreeMap<Slot, Arc<Skin>>,
) -> anyhow::Result<Done> {
    let existing = active.get(&slot);
    let path = || {
        chosen
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("skin: no file selected"))
    };
    let first = || anyhow::anyhow!("skin: import a character first");
    let (skin, skipped) = match action {
        Action::Reset => {
            files::reset(folder, slot)?;
            return Ok(Done::Reset);
        }
        Action::Export(meta) => {
            meta.validate()?;
            let target = path()?;
            let Some(existing) = existing else {
                let mut layout = Layout::default();
                if slot == Slot::Whale {
                    layout.y = 30.0;
                }
                let builtin = Skin::new(slot, crate::agents::assets::builtin(slot), layout)?;
                let builtin = builtin.with_meta(Some(meta));
                files::atomic_write(target, &files::document(slot, &builtin)?)?;
                return Ok(Done::Exported(None));
            };
            let skin = Skin::clone(existing).with_meta(Some(meta));
            files::atomic_write(target, &files::document(slot, &skin)?)?;
            files::save(folder, slot, &skin)?;
            return Ok(Done::Exported(Some(Arc::new(skin))));
        }
        Action::Import => files::read_pack(path()?, slot)?,
        Action::Image => {
            let idle = BTreeMap::from([("idle".into(), files::read_image(path()?)?)]);
            let meta = existing.and_then(|skin| skin.meta.clone());
            let skin = Skin::new(slot, idle, Layout::default())?.with_meta(meta);
            (skin, Vec::new())
        }
        Action::Pose(pose) => {
            let existing = existing.ok_or_else(first)?;
            let mut images = existing.bytes.clone();
            images.insert(pose, files::read_image(path()?)?);
            let skin = Skin::new(slot, images, existing.layout)?.with_meta(existing.meta.clone());
            (skin, Vec::new())
        }
        Action::Layout(layout) => (existing.ok_or_else(first)?.with_layout(layout)?, Vec::new()),
    };
    skins::room(&active, slot, &skin)?;
    files::save(folder, slot, &skin)?;
    Ok(Done::Applied(Arc::new(skin), skipped))
}

fn finish(slot: Slot, result: anyhow::Result<Done>, cx: &mut App) {
    cx.update_global::<Library, _>(|library, _| {
        library.busy = false;
        match result {
            Ok(done) => {
                library.notice = Some(match done {
                    Done::Applied(skin, skipped) => {
                        library.active.insert(slot, skin);
                        library.skipped = skipped;
                        "et.characters.applied"
                    }
                    Done::Reset => {
                        library.active.remove(&slot);
                        "et.characters.restored"
                    }
                    Done::Exported(skin) => {
                        if let Some(skin) = skin {
                            library.active.insert(slot, skin);
                        }
                        "et.characters.exported"
                    }
                });
                log::info!("characters: operation completed for {}", slot.id());
            }
            Err(error) => {
                log::error!("characters: {}: {error:#}", slot.id());
                library.error = Some(format!("{error:#}"));
            }
        }
    });
    cx.refresh_windows();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_operations_import_replace_share_reject_and_restore_without_losing_saved_art() {
        let root = std::env::temp_dir().join(format!("ely-skin-settings-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("source.webp");
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/agents/kimi-idle.webp"),
            &source,
        )
        .unwrap();
        let mut active = BTreeMap::new();
        let applied = execute(
            &root,
            Slot::Claude,
            Action::Image,
            Some(source.clone()),
            active.clone(),
        )
        .unwrap();
        let Done::Applied(skin, _) = applied else {
            panic!("applied skin")
        };
        active.insert(Slot::Claude, skin);
        let applied = execute(
            &root,
            Slot::Claude,
            Action::Pose("thinking".into()),
            Some(source.clone()),
            active.clone(),
        )
        .unwrap();
        let Done::Applied(skin, _) = applied else {
            panic!("applied pose")
        };
        assert!(skin.bytes.contains_key("thinking"));
        active.insert(Slot::Claude, skin);
        std::fs::remove_file(source).unwrap();
        let share = root.join("share.elyskin");
        let meta = Meta {
            id: Some("tester.orange".into()),
            name: "Orange".into(),
            author: "Tester".into(),
            description: None,
            license: Some("CC-BY-4.0".into()),
            homepage: None,
        };
        let unnamed = Meta {
            author: " ".into(),
            ..meta.clone()
        };
        assert!(
            execute(
                &root,
                Slot::Claude,
                Action::Export(unnamed),
                Some(share.clone()),
                active.clone()
            )
            .is_err()
        );
        let exported = execute(
            &root,
            Slot::Claude,
            Action::Export(meta.clone()),
            Some(share.clone()),
            active.clone(),
        )
        .unwrap();
        let Done::Exported(Some(skin)) = exported else {
            panic!("exported with credits")
        };
        assert_eq!(skin.meta.as_ref(), Some(&meta));
        active.insert(Slot::Claude, skin);
        let applied = execute(
            &root,
            Slot::Gpt,
            Action::Import,
            Some(share.clone()),
            active.clone(),
        )
        .unwrap();
        let Done::Applied(skin, _) = applied else {
            panic!("imported skin")
        };
        assert_eq!(skin.bytes, active[&Slot::Claude].bytes);
        assert_eq!(skin.meta.as_ref(), Some(&meta));
        active.insert(Slot::Gpt, skin);
        let saved = std::fs::read(files::path(&root, Slot::Gpt)).unwrap();
        std::fs::write(&share, b"broken pack").unwrap();
        assert!(
            execute(
                &root,
                Slot::Gpt,
                Action::Import,
                Some(share),
                active.clone()
            )
            .is_err()
        );
        assert_eq!(std::fs::read(files::path(&root, Slot::Gpt)).unwrap(), saved);
        assert!(matches!(
            execute(&root, Slot::Claude, Action::Reset, None, active).unwrap(),
            Done::Reset
        ));
        assert!(!files::path(&root, Slot::Claude).exists());
        assert!(files::path(&root, Slot::Gpt).exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
