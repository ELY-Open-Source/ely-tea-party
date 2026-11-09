use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};

use ely_gpui_component::lists::GitStatus;
use gpui::{AppContext, Context, EventEmitter, Pixels, Point, SharedString, Task};

use crate::settings::{PanelTab, SettingsStore};

mod actions;
mod icons;
mod marks;
mod ops;
mod tree;

use marks::{Marked, mark_key, remark};
pub use tree::FileDrag;

/// How often Files reads its open folders again.
const POLL: Duration = Duration::from_secs(1);

/// A folder's entry: name, whether a folder, and modified time.
#[derive(Debug, PartialEq)]
struct Entry {
    name: OsString,
    folder: bool,
    modified: SystemTime,
}

/// A folder's entries, or why it could not be read.
type Listing = Result<Vec<Entry>, String>;

/// Reads `folder`: folders first, then files, each by name ignoring case.
fn list(folder: &Path) -> Listing {
    let read = || -> io::Result<Vec<Entry>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(folder)? {
            let entry = entry?;
            // An entry gone since the folder listed it is gone.
            let meta = match entry.metadata() {
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                meta => meta?,
            };
            let kind = meta.file_type();
            entries.push(Entry {
                folder: kind.is_dir() || (kind.is_symlink() && entry.path().is_dir()),
                name: entry.file_name(),
                modified: meta.modified()?,
            });
        }
        Ok(entries)
    };
    let mut entries = read().map_err(|error| error.to_string())?;
    entries.sort_by_cached_key(|entry| {
        let name = entry.name.to_string_lossy().to_lowercase();
        (!entry.folder, name, entry.name.clone())
    });
    Ok(entries)
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

/// New session here's folder: itself, or a file's parent.
pub(super) fn here_folder(path: &Path, folder: bool) -> PathBuf {
    if folder {
        path.to_path_buf()
    } else {
        path.parent()
            .expect("a listed file lies in a folder")
            .to_path_buf()
    }
}

/// New session here: the folder it starts in.
pub struct Here(pub PathBuf);

/// The words for showing a path in the system's file manager.
pub const REVEAL: &str = if cfg!(target_os = "macos") {
    "et.files.reveal_finder"
} else {
    "et.files.reveal"
};

/// A right press waiting to open Files' menu.
struct MenuAsk {
    n: u64,
    /// The press's point, until a render hands it on.
    at: Option<Point<Pixels>>,
    path: PathBuf,
    folder: bool,
}

/// The Files panel: the shown session's folder as a tree, Git's marks on names.
pub struct Files {
    root: PathBuf,
    /// Counts roots, so each root starts a fresh tree.
    generation: usize,
    listings: HashMap<PathBuf, Listing>,
    marks: HashMap<PathBuf, GitStatus>,
    /// Folders holding a mark.
    dirty: HashSet<PathBuf>,
    /// Git's changes as last marked, by path.
    changes: Arc<Vec<(PathBuf, GitStatus)>>,
    /// Why Git last failed, while it fails.
    failing: Option<String>,
    reading: Task<()>,
    /// A read under way, so a poll waits for it.
    loading: bool,
    checking: Task<()>,
    tree: tree::State,
    creating: Option<actions::Creation>,
    /// A create or a trash under way.
    busy: bool,
    error: Option<String>,
    /// A Git check running, and whether another follows it.
    marking: (bool, bool),
    polling: Task<()>,
}

impl EventEmitter<Here> for Files {}

impl Files {
    pub fn new(root: PathBuf) -> Self {
        Self {
            tree: tree::State::new(root.clone()),
            creating: None,
            busy: false,
            error: None,
            root,
            generation: 0,
            listings: HashMap::new(),
            marks: HashMap::new(),
            dirty: HashSet::new(),
            changes: Arc::default(),
            failing: None,
            reading: Task::ready(()),
            loading: false,
            checking: Task::ready(()),
            marking: (false, false),
            polling: Task::ready(()),
        }
    }

    /// Reads open folders and Git's marks each second, shown.
    pub fn poll(&mut self, cx: &mut Context<Self>) {
        self.polling = cx.spawn(async move |files, cx| {
            loop {
                cx.background_executor().timer(POLL).await;
                if files.update(cx, |files, cx| files.tick(cx)).is_err() {
                    break;
                }
            }
        });
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let settings = SettingsStore::get(cx);
        let showing = settings.show_panel && settings.panel == PanelTab::Files;
        if !showing {
            return;
        }
        if !self.loading {
            self.read(cx);
        }
        // A change deep in a closed folder moves no listing.
        if !self.marking.0 {
            self.check(cx);
        }
    }

    /// Open folders whose folders above are open too.
    fn shown(&self) -> Vec<PathBuf> {
        let within = |folder: &&PathBuf| {
            let up = folder.ancestors().skip(1);
            up.take_while(|above| above.starts_with(&self.root))
                .all(|above| self.tree.open.contains(above))
        };
        let shown = self.tree.open.iter().filter(within).cloned();
        // The root, closed too, so one gone and back shows.
        let root = std::iter::once(self.root.clone());
        root.chain(shown.filter(|folder| *folder != self.root))
            .collect()
    }

    /// Shows `folder`, read afresh with its marks; a new folder starts a new tree.
    pub fn show(&mut self, folder: &Path, cx: &mut Context<Self>) {
        if folder != self.root {
            log::info!("files: the tree shows {}", folder.display());
            let polling = std::mem::replace(&mut self.polling, Task::ready(()));
            *self = Self {
                generation: self.generation + 1,
                polling,
                ..Self::new(folder.to_path_buf())
            };
        }
        self.read(cx);
        self.check(cx);
    }

    /// Reads Git's marks in the background, one at a time.
    fn check(&mut self, cx: &mut Context<Self>) {
        if self.marking.0 {
            self.marking.1 = true;
            return;
        }
        self.marking = (true, false);
        let (root, last) = (self.root.clone(), self.changes.clone());
        let marked = cx.background_spawn(async move { remark(&root, &last) });
        self.checking = cx.spawn(async move |files, cx| {
            let marked = marked.await;
            files.update(cx, |files, cx| files.mark(marked, cx)).ok();
        });
    }

    /// Wears new marks, then runs a check asked meanwhile.
    fn mark(&mut self, marked: Result<Option<Marked>, String>, cx: &mut Context<Self>) {
        let root = self.root.display();
        // A failure keeps the last marks, logged once.
        let marked = match marked {
            Err(why) => {
                if self.failing.as_ref() != Some(&why) {
                    log::error!("files: no Git marks in {root}, the last kept: {why}");
                }
                self.failing = Some(why);
                None
            }
            Ok(marked) => {
                if self.failing.take().is_some() {
                    log::info!("files: Git marks {root} again");
                }
                marked
            }
        };
        if let Some((changes, marks, dirty)) = marked {
            log::debug!(
                "files: {} Git marks in {}",
                marks.len(),
                self.root.display()
            );
            (self.changes, self.marks, self.dirty) = (changes, marks, dirty);
            cx.notify();
        }
        let (_, again) = std::mem::take(&mut self.marking);
        if again {
            self.check(cx);
        }
    }

    /// Reads the root and open folders off the main thread.
    fn read(&mut self, cx: &mut Context<Self>) {
        let folders = self.shown();
        let read = cx.background_spawn(async move {
            let read = folders.into_iter().map(|folder| (list(&folder), folder));
            read.collect::<Vec<_>>()
        });
        self.loading = true;
        self.reading = cx.spawn(async move |files, cx| {
            let read = read.await;
            files
                .update(cx, |files, cx| {
                    files.loading = false;
                    files.store(read, cx);
                })
                .ok();
        });
    }

    /// Keeps what was read.
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
            self.prune();
            cx.notify();
        }
    }

    /// Drops each folder, open or read before, that is gone.
    fn prune(&mut self) {
        let mut gone: Vec<PathBuf> = (self.tree.open.iter().chain(self.listings.keys()))
            .filter(|folder| self.gone(folder))
            .cloned()
            .collect();
        gone.sort();
        gone.dedup();
        for folder in &gone {
            log::debug!("files: {} is gone", folder.display());
            self.tree.open.remove(folder);
            self.listings.remove(folder);
        }
    }

    /// Whether a listing no longer names `folder` or one above.
    fn gone(&self, folder: &Path) -> bool {
        let mut up = folder.ancestors().take_while(|path| *path != self.root);
        up.any(|path| {
            let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
                return false;
            };
            match self.listings.get(parent) {
                Some(Ok(entries)) => !entries.iter().any(|e| e.folder && e.name == name),
                // An unreadable folder, a deleted root too, shows nothing.
                Some(Err(_)) => true,
                None => false,
            }
        })
    }

    /// The tree opened `folder`, unread so far.
    fn load(&mut self, folder: PathBuf, cx: &mut Context<Self>) {
        self.tree.open.insert(folder);
        self.read(cx);
    }

    /// The menu's request, once: a remounted menu would replay it.
    fn request(&mut self) -> Option<(u64, Point<Pixels>)> {
        let ask = self.tree.menu.as_mut()?;
        Some((ask.n, ask.at.take()?))
    }

    /// The path a tree key stands for: the root, or an entry read.
    pub fn path(&self, of: &str) -> Option<PathBuf> {
        self.named(of).map(|(path, _)| path)
    }

    /// A tree key's path, and whether it is a folder.
    fn named(&self, of: &str) -> Option<(PathBuf, bool)> {
        if key(&self.root).as_ref() == of {
            return Some((self.root.clone(), true));
        }
        self.listings.iter().find_map(|(folder, listing)| {
            listing.iter().flatten().find_map(|entry| {
                let path = folder.join(&entry.name);
                (key(&path).as_ref() == of).then_some((path, entry.folder))
            })
        })
    }
}

#[cfg(test)]
mod tests;
