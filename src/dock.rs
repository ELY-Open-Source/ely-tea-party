//! Now and then, while the app runs, the Dock's icon shows the whale girl biting the terminal for a few seconds: an easter egg. Finder, Launchpad and a closed app keep the bundle's icon.

use std::{
    collections::hash_map::RandomState,
    hash::{BuildHasher, Hasher},
    ops::RangeInclusive,
    time::Duration,
};

use gpui::App;
use objc::{class, msg_send, runtime::Object, sel, sel_impl};

/// The bundle's icon with her biting, the same shape and size, from `scripts/icon.py --bite`.
const BITE: &[u8] = include_bytes!("../assets/icons/ely-tea-party-bite.png");
/// Seconds between her bites, picked anew each time, and how long a bite shows.
const EVERY: RangeInclusive<u64> = 600..=1800;
const HELD: Duration = Duration::from_secs(5);

/// Starts her bites, on the main thread, where AppKit takes the icon.
pub fn start(cx: &mut App) {
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(between()).await;
            if let Err(error) = wear(true) {
                log::error!("{error:#}");
                break;
            }
            log::info!("dock: the whale girl bites the terminal");
            cx.background_executor().timer(HELD).await;
            if let Err(error) = wear(false) {
                log::error!("{error:#}");
                break;
            }
        }
    })
    .detach();
}

/// A wait in `EVERY`, from the random keys std seeds each hasher with.
fn between() -> Duration {
    let roll = RandomState::new().build_hasher().finish();
    Duration::from_secs(EVERY.start() + roll % (EVERY.end() - EVERY.start() + 1))
}

/// Shows her bite as the app's icon, or gives the bundle's back.
fn wear(bite: bool) -> anyhow::Result<()> {
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        if !bite {
            let _: () = msg_send![app, setApplicationIconImage: std::ptr::null_mut::<Object>()];
            return Ok(());
        }
        let data: *mut Object =
            msg_send![class!(NSData), dataWithBytes: BITE.as_ptr() length: BITE.len()];
        let image: *mut Object = msg_send![class!(NSImage), alloc];
        let image: *mut Object = msg_send![image, initWithData: data];
        anyhow::ensure!(!image.is_null(), "dock: her bite does not decode");
        let _: () = msg_send![app, setApplicationIconImage: image];
        let _: () = msg_send![image, release];
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_wait_falls_between_ten_and_thirty_minutes() {
        for _ in 0..200 {
            assert!(EVERY.contains(&between().as_secs()));
        }
    }

    #[test]
    fn her_bite_is_a_square_png_at_the_iconset_s_256_point_size() {
        let bite = image::load_from_memory(BITE).expect("decodes");
        assert_eq!((bite.width(), bite.height()), (512, 512));
    }
}
