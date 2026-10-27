//! The characters' animations, built in as animated WebP and decoded frame by frame, so they move whether or not the window is active: gpui's `img` animates only in an active one.

use std::{
    collections::HashMap,
    io::Cursor,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use gpui::{App, RenderImage};
use image::{AnimationDecoder, DynamicImage, Frame, Rgba, codecs::webp::WebPDecoder};

use super::{Kind, Pose};

macro_rules! poses {
    ($($kind:literal),*) => {
        &[$(
            poses!(@one $kind, "working"),
            poses!(@one $kind, "working-research"),
            poses!(@one $kind, "working-special"),
            poses!(@one $kind, "working-slack"),
            poses!(@one $kind, "thinking"),
            poses!(@one $kind, "needs_input"),
            poses!(@one $kind, "idle"),
            poses!(@one $kind, "sleeping"),
        )*]
    };
    (@one $kind:literal, $pose:literal) => {
        (
            concat!($kind, "-", $pose),
            include_bytes!(concat!("../../assets/agents/", $kind, "-", $pose, ".webp")),
        )
    };
}

/// Each character's poses as animated WebP, by `KIND-POSE`.
static ART: &[(&str, &[u8])] = poses!(
    "claude", "gpt", "deepseek", "grok", "kimi", "gemini", "pi", "opencode"
);

macro_rules! mascot {
    ($($pose:literal),*) => {
        &[$((
            concat!("whale-", $pose),
            include_bytes!(concat!("../../assets/mascot/whale-", $pose, ".webp")),
        )),*]
    };
}

/// The whale girl's still and poses at twice the rows' size, for the mascot, by `whale-NAME`.
static MASCOT: &[(&str, &[u8])] = mascot!(
    "calm",
    "idle",
    "working",
    "working-special",
    "working-slack",
    "thinking",
    "needs_input",
    "sleeping",
    "walking",
    "walking-left",
    "puzzled-down",
    "puzzled-up"
);

macro_rules! trinkets {
    ($($kind:literal),*) => {
        &[$((
            $kind,
            include_bytes!(concat!("../../assets/trinkets/", $kind, ".webp")),
        )),*]
    };
}

/// Each character's trinket box: a still WebP strip of `TRINKETS` square cells, by kind.
static BOXES: &[(&str, &[u8])] = trinkets!(
    "claude", "gpt", "deepseek", "grok", "kimi", "gemini", "pi", "opencode"
);

/// How many trinkets each character's box holds.
pub const TRINKETS: usize = 12;

/// One pose's loop: a still per frame, and when each frame ends.
struct Strip {
    frames: Vec<Arc<RenderImage>>,
    ends: Vec<Duration>,
}

/// `frame` in BGRA, which gpui draws.
fn bgra(mut frame: Frame) -> Frame {
    for pixel in frame.buffer_mut().chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    frame
}

impl Strip {
    fn decode(bytes: &[u8]) -> anyhow::Result<Strip> {
        let mut decoder = WebPDecoder::new(Cursor::new(bytes))?;
        if !decoder.has_animation() {
            let still = DynamicImage::from_decoder(decoder)?.into_rgba8();
            let frame = bgra(Frame::new(still));
            return Ok(Strip {
                frames: vec![Arc::new(RenderImage::new([frame]))],
                ends: vec![Duration::ZERO],
            });
        }
        decoder.set_background_color(Rgba([0, 0, 0, 0]))?;
        let (mut frames, mut ends, mut end) = (Vec::new(), Vec::new(), Duration::ZERO);
        for frame in decoder.into_frames() {
            let frame = frame?;
            let (numer, denom) = frame.delay().numer_denom_ms();
            end += Duration::from_secs_f64(f64::from(numer) / f64::from(denom.max(1)) / 1000.0);
            frames.push(Arc::new(RenderImage::new([bgra(frame)])));
            ends.push(end);
        }
        anyhow::ensure!(!frames.is_empty(), "no frames");
        Ok(Strip { frames, ends })
    }

    /// The frame `at` into the loop.
    fn at(&self, at: Duration) -> Arc<RenderImage> {
        let total = self.ends.last().copied().unwrap_or_default();
        let into = if total.is_zero() {
            Duration::ZERO
        } else {
            Duration::from_nanos((at.as_nanos() % total.as_nanos()) as u64)
        };
        let index = self.ends.partition_point(|end| *end <= into);
        self.frames[index.min(self.frames.len() - 1)].clone()
    }
}

/// The loops decoded so far, each the first time it shows, by name.
type Strips = Mutex<HashMap<&'static str, Arc<Strip>>>;

fn strips() -> &'static Strips {
    static STRIPS: OnceLock<Strips> = OnceLock::new();
    STRIPS.get_or_init(Default::default)
}

/// `kind` in `pose`, the frame for `now`; `seed` staggers sessions so they move apart.
pub fn frame(
    kind: Kind,
    pose: Pose,
    seed: u64,
    now: Instant,
    cx: &App,
) -> Option<Arc<RenderImage>> {
    if let Some(skin) = crate::skins::custom(crate::skins::Slot::agent(kind), cx) {
        return Some(skin.frame(pose.stem(), seed, now));
    }
    let name = format!("{}-{}", kind.stem(), pose.stem());
    play(ART, &name, seed, now)
}

/// The mascot's loop `name`, the frame for `now`; none for one she lacks.
pub fn mascot(name: &str, now: Instant, cx: &App) -> Option<Arc<RenderImage>> {
    if let Some(skin) = crate::skins::custom(crate::skins::Slot::Whale, cx) {
        return Some(skin.frame(name, 0, now));
    }
    play(MASCOT, &format!("whale-{name}"), 0, now)
}

pub fn builtin(slot: crate::skins::Slot) -> std::collections::BTreeMap<String, Arc<[u8]>> {
    let art = if slot == crate::skins::Slot::Whale {
        MASCOT
    } else {
        ART
    };
    let prefix = format!("{}-", slot.id());
    art.iter()
        .filter_map(|(name, bytes)| {
            name.strip_prefix(&prefix)
                .map(|pose| (pose.to_string(), Arc::from(*bytes)))
        })
        .collect()
}

/// Trinket `index` from `kind`'s box, the box cut into its cells the first time.
pub fn trinket(kind: Kind, index: usize) -> Option<Arc<RenderImage>> {
    type Boxes = Mutex<HashMap<&'static str, Arc<Vec<Arc<RenderImage>>>>>;
    static CUT: OnceLock<Boxes> = OnceLock::new();
    let (name, bytes) = BOXES.iter().find(|(name, _)| *name == kind.stem())?;
    let mut cut = CUT.get_or_init(Default::default).lock().ok()?;
    let cells = match cut.get(name) {
        Some(cells) => cells.clone(),
        None => match cells(bytes) {
            Ok(cells) => {
                let cells = Arc::new(cells);
                cut.insert(name, cells.clone());
                cells
            }
            Err(error) => {
                log::error!("agents: {name}'s trinkets do not decode: {error:#}");
                return None;
            }
        },
    };
    cells.get(index % TRINKETS).cloned()
}

/// A box's strip cut into its square cells.
fn cells(bytes: &[u8]) -> anyhow::Result<Vec<Arc<RenderImage>>> {
    let strip = image::load_from_memory_with_format(bytes, image::ImageFormat::WebP)?.into_rgba8();
    let side = strip.height();
    anyhow::ensure!(
        strip.width() == side * TRINKETS as u32,
        "a strip of {TRINKETS} square cells, not {} by {side}",
        strip.width()
    );
    Ok((0..TRINKETS as u32)
        .map(|cell| {
            let cell = image::imageops::crop_imm(&strip, cell * side, 0, side, side).to_image();
            Arc::new(RenderImage::new([bgra(Frame::new(cell))]))
        })
        .collect())
}

/// The frame of `art`'s loop `name` for `now`, decoding it the first time.
fn play(
    art: &'static [(&'static str, &'static [u8])],
    name: &str,
    seed: u64,
    now: Instant,
) -> Option<Arc<RenderImage>> {
    static START: OnceLock<Instant> = OnceLock::new();
    let start = *START.get_or_init(|| now);
    let (name, bytes) = art.iter().find(|(art, _)| *art == name)?;
    let mut strips = strips().lock().ok()?;
    let strip = match strips.get(name) {
        Some(strip) => strip.clone(),
        None => match Strip::decode(bytes) {
            Ok(strip) => {
                log::debug!("agents: {name} decoded, {} frames", strip.frames.len());
                let strip = Arc::new(strip);
                strips.insert(name, strip.clone());
                strip
            }
            Err(error) => {
                log::error!("agents: {name} does not decode: {error:#}");
                return None;
            }
        },
    };
    let at = now.saturating_duration_since(start) + Duration::from_millis(seed * 617);
    Some(strip.at(at))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pose_decodes_into_a_loop() {
        assert_eq!(ART.len(), Kind::ALL.len() * Pose::ALL.len());
        for kind in Kind::ALL {
            for pose in Pose::ALL {
                let name = format!("{}-{}", kind.stem(), pose.stem());
                let (_, bytes) = ART.iter().find(|(art, _)| *art == name).expect("built in");
                let strip = Strip::decode(bytes).expect("decodes");
                assert!(strip.frames.len() > 1, "{name} moves");
            }
        }
    }

    #[test]
    fn the_mascot_has_every_pose_she_plays_and_twice_the_rows_size() {
        for (name, bytes) in MASCOT {
            let strip = Strip::decode(bytes).expect("decodes");
            let moves = *name != "whale-calm";
            assert_eq!(strip.frames.len() > 1, moves, "{name} moves, but her still");
            assert_eq!(strip.frames[0].size(0).width.0, 224, "{name}");
        }
    }

    #[test]
    fn every_character_has_a_full_box_of_trinkets() {
        assert_eq!(BOXES.len(), Kind::ALL.len());
        for kind in Kind::ALL {
            let (_, bytes) = BOXES
                .iter()
                .find(|(name, _)| *name == kind.stem())
                .expect("built in");
            let cells = cells(bytes).expect("decodes");
            assert_eq!(cells.len(), TRINKETS);
            for (index, cell) in cells.iter().enumerate() {
                let drawn = cell
                    .as_bytes(0)
                    .expect("a frame")
                    .chunks_exact(4)
                    .any(|pixel| pixel[3] > 0);
                assert!(drawn, "{}'s trinket {index} is empty", kind.stem());
            }
        }
    }

    #[test]
    fn frames_follow_the_clock_round_the_loop() {
        let (_, bytes) = ART[0];
        let strip = Strip::decode(bytes).expect("decodes");
        let total = *strip.ends.last().expect("a frame");
        assert_eq!(strip.at(Duration::ZERO).id, strip.frames[0].id);
        assert_eq!(strip.at(strip.ends[0]).id, strip.frames[1].id);
        assert_eq!(strip.at(total).id, strip.frames[0].id, "round again");
    }
}
