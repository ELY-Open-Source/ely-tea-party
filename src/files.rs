use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use ely_gpui_component::lists::GitStatus;
use gpui::{AppContext, Context, SharedString, Task};

use crate::git;

mod actions;
mod icons;
mod ops;
mod tree;

/// A folder's entry: its name, and whether it is a folder, links followed.
#[derive(Debug, PartialEq)]
struct Entry {
    name: OsString,
    folder: bool,
}

/// A folder's entries, or why it could not be read.
type Listing = Result<Vec<Entry>, String>;

/// Reads `folder`: folders first, then files, each by name ignoring case.
fn list(folder: &Path) -> Listing {
    let read = || -> io::Result<Vec<Entry>> {
        fs::read_dir(folder)?
            .map(|entry| {
                let entry = entry?;
                let kind = entry.file_type()?;
                Ok(Entry {
                    folder: kind.is_dir() || (kind.is_symlink() && entry.path().is_dir()),
                    name: entry.file_name(),
                })
            })
            .collect()
    };
    let mut entries = read().map_err(|error| error.to_string())?;
    entries.sort_by_cached_key(|entry| {
        let name = entry.name.to_string_lossy().to_lowercase();
        (!entry.folder, name, entry.name.clone())
    });
    Ok(entries)
}

/// `path` as marks are keyed: on macOS, whose file systems ignore Unicode forms, composed as Git writes it.
fn mark_key(path: &Path) -> Cow<'_, Path> {
    #[cfg(target_os = "macos")]
    if let Some(text) = path.to_str()
        && !unicode_normalization::is_nfc(text)
    {
        use unicode_normalization::UnicodeNormalization;
        return Cow::Owned(text.nfc().collect::<String>().into());
    }
    Cow::Borrowed(path)
}

/// Changes by their keys, and the folders from `root` down that hold one.
fn marks(
    root: &Path,
    changes: Vec<(PathBuf, GitStatus)>,
) -> (HashMap<PathBuf, GitStatus>, HashSet<PathBuf>) {
    let root = mark_key(root);
    let marks: HashMap<PathBuf, GitStatus> = changes
        .into_iter()
        .map(|(path, status)| (mark_key(&path).into_owned(), status))
        .collect();
    let dirty = marks
        .keys()
        .flat_map(|path| {
            let up = path.ancestors().skip(1);
            up.take_while(|folder| folder.starts_with(&root))
        })
        .map(Path::to_path_buf)
        .collect();
    (marks, dirty)
}

/// `path`'s tree key, one to a path: a path not in UTF-8 adds its bytes after a NUL, which no path holds.
fn key(path: &Path) -> SharedString {
    match path.to_str() {
        Some(text) => text.to_string().into(),
        None => {
            let bytes = path.as_os_str().as_encoded_bytes();
            let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
            format!("{}\0{hex}", path.to_string_lossy()).into()
        }
    }
}

/// The Files panel: the shown session's folder as a tree, Git's marks on names.
pub struct Files {
    root: PathBuf,
    /// Counts roots, so each root starts a fresh tree.
    generation: usize,
    listings: HashMap<PathBuf, Listing>,
    /// The root and the folders the tree opened.
    wanted: HashSet<PathBuf>,
    marks: HashMap<PathBuf, GitStatus>,
    /// Folders holding a mark.
    dirty: HashSet<PathBuf>,
    reading: Task<()>,
    checking: Task<()>,
    tree: tree::State,
    creating: Option<actions::Creation>,
    busy: bool,
    error: Option<String>,
}

impl Files {
    pub fn new(root: PathBuf) -> Self {
        Self {
            wanted: HashSet::from([root.clone()]),
            tree: tree::State::new(root.clone()),
            creating: None,
            busy: false,
            error: None,
            root,
            generation: 0,
            listings: HashMap::new(),
            marks: HashMap::new(),
            dirty: HashSet::new(),
            reading: Task::ready(()),
            checking: Task::ready(()),
        }
    }

    /// Shows `folder`, read afresh with its marks; a new folder starts a new tree.
    pub fn show(&mut self, folder: &Path, cx: &mut Context<Self>) {
        if folder != self.root {
            log::info!("files: the tree shows {}", folder.display());
            *self = Self {
                generation: self.generation + 1,
                ..Self::new(folder.to_path_buf())
            };
        }
        self.read(cx);
        let root = self.root.clone();
        let changes = cx.background_spawn(async move { git::changes(&root) });
        self.checking = cx.spawn(async move |files, cx| {
            let changes = changes.await;
            files.update(cx, |files, cx| files.mark(changes, cx)).ok();
        });
    }

    fn mark(
        &mut self,
        changes: anyhow::Result<Option<Vec<(PathBuf, GitStatus)>>>,
        cx: &mut Context<Self>,
    ) {
        let changes = match changes {
            Ok(changes) => changes.unwrap_or_default(),
            Err(error) => {
                log::error!("files: no Git marks in {}: {error:#}", self.root.display());
                Vec::new()
            }
        };
        let (marks, dirty) = marks(&self.root, changes);
        if marks != self.marks {
            log::debug!(
                "files: {} Git marks in {}",
                marks.len(),
                self.root.display()
            );
        }
        (self.marks, self.dirty) = (marks, dirty);
        cx.notify();
    }

    /// Reads the root and every folder the tree opened, off the main thread.
    fn read(&mut self, cx: &mut Context<Self>) {
        let folders: Vec<PathBuf> = self.wanted.iter().cloned().collect();
        let read = cx.background_spawn(async move {
            let read = folders.into_iter().map(|folder| (list(&folder), folder));
            read.collect::<Vec<_>>()
        });
        self.reading = cx.spawn(async move |files, cx| {
            let read = read.await;
            files.update(cx, |files, cx| files.store(read, cx)).ok();
        });
    }

    fn store(&mut self, read: Vec<(Listing, PathBuf)>, cx: &mut Context<Self>) {
        let mut changed = false;
        for (listing, folder) in read {
            if self.listings.get(&folder) == Some(&listing) {
                continue;
            }
            match &listing {
                Ok(entries) => {
                    log::debug!("files: {} holds {}", folder.display(), entries.len())
                }
                Err(error) => log::warn!("files: {} is unreadable: {error}", folder.display()),
            }
            self.listings.insert(folder, listing);
            changed = true;
        }
        if changed {
            cx.notify();
        }
    }

    /// The tree opened `folder`, unread so far.
    fn load(&mut self, folder: PathBuf, cx: &mut Context<Self>) {
        if self.wanted.insert(folder) {
            self.read(cx);
        }
    }
}

#[cfg(test)]
mod tests;
