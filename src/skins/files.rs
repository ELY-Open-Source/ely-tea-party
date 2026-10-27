use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context as _;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use super::{Layout, Skin, Slot};

pub const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PACK_BYTES: usize = 32 * 1024 * 1024;
const MAX_DOCUMENT_BYTES: usize = 44 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pack {
    version: u32,
    target: Slot,
    #[serde(default)]
    layout: Layout,
    images: BTreeMap<String, String>,
}

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

pub fn read_pack(path: &Path, slot: Slot) -> anyhow::Result<Skin> {
    let pack: Pack = serde_json::from_slice(&read_limited(path, MAX_DOCUMENT_BYTES)?)
        .context("skin: invalid character pack")?;
    anyhow::ensure!(
        pack.version == 1,
        "skin: unsupported pack version {}",
        pack.version
    );
    anyhow::ensure!(
        (pack.target == Slot::Whale) == (slot == Slot::Whale),
        "skin: this pack is for {}, select that character first",
        pack.target.id()
    );
    anyhow::ensure!(
        pack.images.len() <= slot.poses().len(),
        "skin: too many actions"
    );
    let mut images = BTreeMap::new();
    for (pose, encoded) in pack.images {
        anyhow::ensure!(
            slot.poses().contains(&pose.as_str()),
            "skin: unknown action {pose}"
        );
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
    Skin::new(slot, images, pack.layout)
}

pub fn document(
    slot: Slot,
    images: &BTreeMap<String, Arc<[u8]>>,
    layout: Layout,
) -> anyhow::Result<Vec<u8>> {
    Ok(serde_json::to_vec(&Pack {
        version: 1,
        target: slot,
        layout,
        images: images
            .iter()
            .map(|(name, bytes)| (name.clone(), STANDARD.encode(bytes)))
            .collect(),
    })?)
}

pub fn save(folder: &Path, slot: Slot, skin: &Skin) -> anyhow::Result<()> {
    fs::create_dir_all(folder).context("skin: cannot create character folder")?;
    atomic_write(
        &path(folder, slot),
        &document(slot, &skin.bytes, skin.layout)?,
    )
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
