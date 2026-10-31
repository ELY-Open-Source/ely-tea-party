use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context as _;
use base64::{Engine as _, engine::general_purpose::STANDARD};

use super::{Skin, Slot, pack};

pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PACK_BYTES: usize = 32 * 1024 * 1024;
const MAX_DOCUMENT_BYTES: usize = 44 * 1024 * 1024;

pub fn path(folder: &Path, slot: Slot) -> PathBuf {
    folder.join(format!("{}.elyskin", slot.id()))
}

pub fn read_image(path: &Path) -> anyhow::Result<Arc<[u8]>> {
    Ok(read_limited(path, MAX_IMAGE_BYTES)?.into())
}

fn read_limited(path: &Path, limit: usize) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .with_context(|| format!("skin: cannot read {}", path.display()))?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() <= limit,
        "skin: {} exceeds {} MiB",
        path.display(),
        limit / 1024 / 1024
    );
    Ok(bytes)
}

/// Reads the pack at `path` for `slot`; the actions this build skipped come back beside it.
pub fn read_pack(path: &Path, slot: Slot) -> anyhow::Result<(Skin, Vec<String>)> {
    let read = pack::read(&read_limited(path, MAX_DOCUMENT_BYTES)?, slot)?;
    let mut images = BTreeMap::new();
    for (pose, encoded) in read.images {
        anyhow::ensure!(
            encoded.len() <= MAX_IMAGE_BYTES.div_ceil(3) * 4,
            "skin: {pose} exceeds 8 MiB"
        );
        images.insert(
            pose,
            STANDARD
                .decode(encoded)
                .context("skin: invalid base64 image")?
                .into(),
        );
    }
    let skin = Skin::new(slot, images, read.layout)?.with_meta(read.meta);
    Ok((skin, read.skipped))
}

/// `skin`'s images, layout and credits as a pack for `slot`.
pub fn document(slot: Slot, skin: &Skin) -> anyhow::Result<Vec<u8>> {
    let images = skin
        .bytes
        .iter()
        .map(|(name, bytes)| (name.as_str(), STANDARD.encode(bytes)))
        .collect();
    pack::write(slot, skin.meta.as_ref(), skin.layout, images)
}

pub fn save(folder: &Path, slot: Slot, skin: &Skin) -> anyhow::Result<()> {
    fs::create_dir_all(folder).context("skin: cannot create character folder")?;
    atomic_write(&path(folder, slot), &document(slot, skin)?)
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().context("skin: destination has no parent")?;
    let temporary = parent.join(format!(".elyskin-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        anyhow::Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.with_context(|| format!("skin: cannot save {}", path.display()))
}

pub fn reset(folder: &Path, slot: Slot) -> anyhow::Result<()> {
    match fs::remove_file(path(folder, slot)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("skin: cannot restore default character"),
    }
}
