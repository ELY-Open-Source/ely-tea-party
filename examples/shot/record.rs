//! Films the main window, its child windows over it, into a video through ffmpeg while the specs play: `SHOT_RECORD=PATH.mp4`, `SHOT_FPS` frames a second.

use std::{
    io::Write,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use anyhow::{Context as _, bail};
use core_graphics::{
    geometry::{CGPoint, CGRect, CGSize},
    window,
};

/// The ground above the window, where only her window shows: Ely Dark's.
const GROUND: [u8; 4] = [0x2e, 0x1e, 0x1e, 0xff];

/// A film in progress.
pub struct Recorder {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<anyhow::Result<usize>>,
}

/// One window's pixels, BGRA rows top down.
struct Picture {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

fn picture(number: u32) -> anyhow::Result<Picture> {
    let image = window::create_image(
        CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(0.0, 0.0)),
        window::kCGWindowListOptionIncludingWindow,
        number,
        window::kCGWindowImageBoundsIgnoreFraming | window::kCGWindowImageBestResolution,
    )
    .context("window capture returned nothing")?;
    if image.bits_per_pixel() != 32 {
        bail!("{} bits per pixel", image.bits_per_pixel());
    }
    let (width, height, stride) = (image.width(), image.height(), image.bytes_per_row());
    let data = image.data();
    let bytes = data.bytes();
    let mut pixels = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        pixels.extend_from_slice(&bytes[y * stride..y * stride + width * 4]);
    }
    Ok(Picture {
        width,
        height,
        pixels,
    })
}

/// Rows a child window's picture starts with that hold AppKit's edge line, not her.
const EDGE: usize = 4;

/// `picture` without its first `rows`.
fn below(mut picture: Picture, rows: usize) -> Picture {
    picture
        .pixels
        .drain(..rows.min(picture.height) * picture.width * 4);
    picture.height = picture.height.saturating_sub(rows);
    picture
}

/// Lays `over` on `canvas` at `(x, y)` pixels, blending by its alpha.
fn lay(canvas: &mut Picture, over: &Picture, (x, y): (i64, i64)) {
    for row in 0..over.height {
        let cy = y + row as i64;
        if cy < 0 || cy >= canvas.height as i64 {
            continue;
        }
        for column in 0..over.width {
            let cx = x + column as i64;
            if cx < 0 || cx >= canvas.width as i64 {
                continue;
            }
            let from = (row * over.width + column) * 4;
            let to = (cy as usize * canvas.width + cx as usize) * 4;
            let alpha = u32::from(over.pixels[from + 3]);
            for channel in 0..3 {
                let (top, under) = (
                    u32::from(over.pixels[from + channel]),
                    u32::from(canvas.pixels[to + channel]),
                );
                // CoreGraphics hands premultiplied pixels.
                canvas.pixels[to + channel] = (top + under * (255 - alpha) / 255).min(255) as u8;
            }
        }
    }
}

/// Starts filming window `main` at `frame`, its `children` laid over it where they stand, to `path`.
pub fn start(
    main: (u32, [f64; 4]),
    children: Vec<(u32, [f64; 4])>,
    path: &str,
    fps: u32,
) -> anyhow::Result<Recorder> {
    let first = picture(main.0)?;
    let scale = first.width as f64 / main.1[2];
    // AppKit's frames rise from the screen's bottom; a child's top above the window's is room on top.
    let top = |at: [f64; 4]| at[1] + at[3];
    let above = children
        .iter()
        .map(|(_, at)| top(*at) - top(main.1))
        .fold(0.0, f64::max);
    let room = ((above * scale).ceil() as usize + 1) & !1;
    let (width, height) = (first.width & !1, (first.height + room) & !1);
    let mut ffmpeg: Child = Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-y",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "bgra",
        ])
        .args(["-s", &format!("{width}x{height}")])
        .args(["-use_wallclock_as_timestamps", "1", "-i", "-"])
        .args(["-fps_mode", "cfr", "-r", &fps.to_string()])
        .args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "18"])
        .args(["-pix_fmt", "yuv420p", "-movflags", "+faststart", path])
        .stdin(Stdio::piped())
        .spawn()
        .context("cannot run ffmpeg")?;
    let mut input = ffmpeg.stdin.take().context("ffmpeg has no stdin")?;
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let every = Duration::from_secs(1) / fps;
    let thread = std::thread::spawn(move || {
        let mut frames = 0;
        while !stopping.load(Ordering::Relaxed) {
            let began = Instant::now();
            let mut canvas = Picture {
                width,
                height,
                pixels: GROUND.repeat(width * height),
            };
            lay(&mut canvas, &picture(main.0)?, (0, room as i64));
            for (child, at) in &children {
                let x = ((at[0] - main.1[0]) * scale).round() as i64;
                let y = room as i64 - ((top(*at) - top(main.1)) * scale).round() as i64;
                lay(
                    &mut canvas,
                    &below(picture(*child)?, EDGE),
                    (x, y + EDGE as i64),
                );
            }
            input
                .write_all(&canvas.pixels)
                .context("ffmpeg took no frame")?;
            frames += 1;
            if let Some(rest) = every.checked_sub(began.elapsed()) {
                std::thread::sleep(rest);
            }
        }
        drop(input);
        let status = ffmpeg.wait()?;
        anyhow::ensure!(status.success(), "ffmpeg ended with {status}");
        Ok(frames)
    });
    log::info!("shot: filming {width}x{height} at {fps} fps into {path}");
    Ok(Recorder { stop, thread })
}

impl Recorder {
    /// Stops filming and waits for the video to be written; the frames filmed.
    pub fn finish(self) -> anyhow::Result<usize> {
        self.stop.store(true, Ordering::Relaxed);
        self.thread
            .join()
            .map_err(|_| anyhow::anyhow!("the film's thread panicked"))?
    }
}
