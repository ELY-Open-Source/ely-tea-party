use std::{
    collections::BTreeMap,
    io::Cursor,
    sync::Arc,
    time::{Duration, Instant},
};

use image::{DynamicImage, Frame, ImageFormat, RgbaImage, codecs::gif::GifEncoder};

use super::*;

fn png(width: u32, height: u32) -> Arc<[u8]> {
    let image = RgbaImage::from_pixel(width, height, image::Rgba([230, 80, 40, 255]));
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut out, ImageFormat::Png)
        .unwrap();
    out.into_inner().into()
}

fn still() -> Skin {
    Skin::new(
        Slot::Claude,
        BTreeMap::from([("idle".into(), png(10, 20))]),
        Layout::default(),
    )
    .unwrap()
}

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("ely-skin-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn a_single_image_is_used_for_every_missing_action_without_builtin_art() {
    let skin = still();
    let now = Instant::now();
    let idle = skin.frame("idle", 0, now);
    for pose in AGENT_POSES {
        assert_eq!(skin.frame(pose, u64::MAX, now).id, idle.id);
    }
    let bytes = idle.as_bytes(0).unwrap();
    assert_eq!(&bytes[..4], &[40, 80, 230, 255]);
    let layout = Layout {
        scale: 0.75,
        x: 2.0,
        y: -3.0,
    };
    let adjusted = skin.with_layout(layout).unwrap();
    assert_eq!(adjusted.frame("idle", 0, now).id, idle.id);
}

#[test]
fn packs_survive_original_file_removal_and_preserve_all_actions_and_layout() {
    let dir = Directory::new();
    let original = dir.0.join("source.png");
    std::fs::write(&original, png(32, 16)).unwrap();
    let bytes = files::read_image(&original).unwrap();
    let layout = Layout {
        scale: 0.8,
        x: -4.0,
        y: 12.0,
    };
    let skin = Skin::new(
        Slot::Whale,
        BTreeMap::from([("idle".into(), bytes.clone()), ("walking".into(), bytes)]),
        layout,
    )
    .unwrap();
    files::save(&dir.0, Slot::Whale, &skin).unwrap();
    std::fs::remove_file(original).unwrap();
    let (loaded, _) = files::read_pack(&files::path(&dir.0, Slot::Whale), Slot::Whale).unwrap();
    assert_eq!(loaded.bytes, skin.bytes);
    assert_eq!(loaded.layout, layout);
    let exported = dir.0.join("share.elyskin");
    files::atomic_write(&exported, &files::document(Slot::Whale, &loaded).unwrap()).unwrap();
    assert_eq!(
        files::read_pack(&exported, Slot::Whale).unwrap().0.bytes,
        skin.bytes
    );
    assert!(files::read_pack(&exported, Slot::Claude).is_err());
    files::reset(&dir.0, Slot::Whale).unwrap();
    assert!(!files::path(&dir.0, Slot::Whale).exists());
    assert!(exported.exists());
}

#[test]
fn every_builtin_character_exports_to_an_importable_pack() {
    let dir = Directory::new();
    for slot in Slot::ALL {
        let images = crate::agents::assets::builtin(slot);
        let skin = Skin::new(slot, images, Layout::default()).unwrap();
        files::save(&dir.0, slot, &skin).unwrap();
        let (loaded, _) = files::read_pack(&files::path(&dir.0, slot), slot).unwrap();
        assert_eq!(loaded.bytes.len(), slot.poses().len());
    }
}

#[test]
fn invalid_images_actions_layouts_and_versions_leave_saved_skin_intact() {
    let dir = Directory::new();
    let path = files::path(&dir.0, Slot::Claude);
    let skin = still();
    files::save(&dir.0, Slot::Claude, &skin).unwrap();
    let saved = std::fs::read(&path).unwrap();
    assert!(Skin::new(Slot::Claude, BTreeMap::new(), Layout::default()).is_err());
    assert!(
        Skin::new(
            Slot::Claude,
            BTreeMap::from([("idle".into(), Arc::from(&b"bad image"[..]))]),
            Layout::default()
        )
        .is_err()
    );
    assert!(Clip::decode(&png(2049, 1)).is_err());
    assert!(Clip::decode(&vec![0; files::MAX_IMAGE_BYTES + 1]).is_err());
    assert!(
        skin.with_layout(Layout {
            scale: f32::NAN,
            ..Layout::default()
        })
        .is_err()
    );
    assert!(
        skin.with_layout(Layout {
            y: 31.0,
            ..Layout::default()
        })
        .is_err()
    );
    let mut document: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    document["images"]["../outside"] = document["images"]["idle"].clone();
    let invalid = dir.0.join("invalid.elyskin");
    std::fs::write(&invalid, serde_json::to_vec(&document).unwrap()).unwrap();
    assert!(files::read_pack(&invalid, Slot::Claude).is_err());
    document["images"]
        .as_object_mut()
        .unwrap()
        .remove("../outside");
    document["version"] = 3.into();
    std::fs::write(&invalid, serde_json::to_vec(&document).unwrap()).unwrap();
    assert!(files::read_pack(&invalid, Slot::Claude).is_err());
    assert_eq!(std::fs::read(path).unwrap(), saved);
    assert!(!dir.0.join("outside").exists());
}

fn gif(count: usize) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut out);
        for i in 0..count {
            let frame = RgbaImage::from_pixel(2, 2, image::Rgba([i as u8, 30, 40, 255]));
            encoder
                .encode_frame(Frame::from_parts(
                    frame,
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(100, 1),
                ))
                .unwrap();
        }
    }
    out
}

#[test]
fn animated_gifs_loop_and_frame_limits_reject_oversized_animations() {
    let clip = Clip::decode(&gif(2)).unwrap();
    assert_ne!(
        clip.at(Duration::ZERO, 0).id,
        clip.at(Duration::from_millis(100), 0).id
    );
    assert_eq!(
        clip.at(Duration::ZERO, 0).id,
        clip.at(Duration::from_millis(200), 0).id
    );
    assert!(Clip::decode(&gif(241)).is_err());
}

#[test]
fn apng_frames_keep_their_timing_and_can_be_used_by_another_agent() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let bytes = STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAACGFjVEwAAAACAAAAAPONk3AAAAAaZmNUTAAAAAAAAAACAAAAAgAAAAAAAAAAAAEACgAA6FTcAAAAABFJREFUeJxj+M/A8B+EGWAMAEfKB/lnWW63AAAAGmZjVEwAAAABAAAAAgAAAAIAAAAAAAAAAAABAAoAAHMnNtQAAAAUZmRBVAAAAAJ4nGNgYPj/H4KhDAA/0gf51gDBTQAAAABJRU5ErkJggg==").unwrap();
    let clip = Clip::decode(&bytes).unwrap();
    assert_eq!(clip.at(Duration::ZERO, 0).size(0).width.0, 2);
    assert_ne!(
        clip.at(Duration::ZERO, 0).id,
        clip.at(Duration::from_millis(100), 0).id
    );
    assert_eq!(
        clip.at(Duration::ZERO, 0).id,
        clip.at(Duration::from_millis(200), 0).id
    );
    let dir = Directory::new();
    files::save(&dir.0, Slot::Claude, &still()).unwrap();
    assert!(files::read_pack(&files::path(&dir.0, Slot::Claude), Slot::Gpt).is_ok());
}
