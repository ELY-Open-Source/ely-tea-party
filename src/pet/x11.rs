//! Ely's window on X11: gpui's popup, which no window manager places, put on the main window's top edge and just above it.

use anyhow::{Context as _, bail};
use gpui::{Bounds, Pixels, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use x11rb::{
    protocol::xproto::{ConfigureWindowAux, ConnectionExt as _, StackMode},
    rust_connection::RustConnection,
};

use super::{HEAD, ROOM, left, pane};

/// Her X window, the main one, and a connection of her own to their server.
pub struct Glue {
    connection: RustConnection,
    pet: u32,
    main: u32,
}

impl Glue {
    pub fn new(pet: &Window, main: &Window) -> anyhow::Result<Self> {
        let (pet, main) = (native(pet)?, native(main)?);
        // gpui opens $DISPLAY too; a window's place belongs to whoever asks.
        let (connection, _) = x11rb::connect(None).context("pet: no X server")?;
        Ok(Self {
            connection,
            pet,
            main,
        })
    }

    /// Stands her on the main window's pane, as far in as its width allows, stacked just above the main window's frame.
    pub fn place(&self, main: &Window) -> anyhow::Result<()> {
        self.put(main.scale_factor(), pane(main))
    }

    /// Places her at `scale` over `pane`, in the main window's points.
    fn put(&self, scale: f32, pane: Bounds<Pixels>) -> anyhow::Result<()> {
        let (x, y) = (f32::from(pane.origin.x), f32::from(pane.origin.y));
        let (root, frame) = self.frame()?;
        let at = (self.connection)
            .translate_coordinates(self.main, root, 0, 0)?
            .reply()
            .context("pet: the main window's place")?;
        let aux = ConfigureWindowAux::new()
            .x(i32::from(at.dst_x) + ((x + left(pane.size.width)) * scale).round() as i32)
            .y(i32::from(at.dst_y) + ((y - HEAD - ROOM) * scale).round() as i32)
            .sibling(frame)
            .stack_mode(StackMode::ABOVE);
        (self.connection)
            .configure_window(self.pet, &aux)?
            .check()
            .context("pet: the X server did not move her window")
    }

    /// Maps her window while the main one shows, and unmaps it while that hides.
    pub fn shown(&self, visible: bool) -> anyhow::Result<()> {
        let cookie = if visible {
            self.connection.map_window(self.pet)?
        } else {
            self.connection.unmap_window(self.pet)?
        };
        cookie
            .check()
            .context("pet: her window did not map or unmap")
    }

    /// The root, and the main window's top window under it: the frame a window manager put round it, or itself.
    fn frame(&self) -> anyhow::Result<(u32, u32)> {
        let mut window = self.main;
        loop {
            let tree = self.connection.query_tree(window)?.reply()?;
            if tree.parent == tree.root {
                return Ok((tree.root, window));
            }
            window = tree.parent;
        }
    }
}

/// The X window gpui made for `window`.
fn native(window: &Window) -> anyhow::Result<u32> {
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("pet: no native handle: {error:?}"))?;
    let RawWindowHandle::Xcb(xcb) = handle.as_raw() else {
        bail!("pet: not an X11 window");
    };
    Ok(xcb.window.get())
}

#[cfg(test)]
mod tests {
    use x11rb::{
        COPY_DEPTH_FROM_PARENT,
        connection::Connection as _,
        protocol::xproto::{CreateWindowAux, MapState, WindowClass},
    };

    use gpui::{point, px, size};

    use super::*;
    use crate::testx11::Xvfb;

    #[test]
    fn she_stands_on_the_pane_just_above_the_frame_and_hides_with_it() {
        let xvfb = Xvfb::start();
        let (connection, screen) = x11rb::connect(Some(&xvfb.display)).expect("the test connects");
        let root = connection.setup().roots[screen].root;
        let window =
            |parent, (x, y, width, height): (i16, i16, u16, u16), aux: &CreateWindowAux| {
                let id = connection.generate_id().expect("a window id");
                let class = WindowClass::INPUT_OUTPUT;
                (connection.create_window(
                    COPY_DEPTH_FROM_PARENT,
                    id,
                    parent,
                    x,
                    y,
                    width,
                    height,
                    0,
                    class,
                    0,
                    aux,
                ))
                .expect("the request")
                .check()
                .expect("a window");
                connection
                    .map_window(id)
                    .expect("the request")
                    .check()
                    .expect("mapped");
                id
            };
        let plain = CreateWindowAux::new();
        let pet = window(
            root,
            (0, 0, 112, 124),
            &CreateWindowAux::new().override_redirect(1),
        );
        // A window manager's frame, its title bar 20 pixels tall, round the main window.
        let frame = window(root, (100, 80, 800, 620), &plain);
        let main = window(frame, (0, 20, 800, 600), &plain);
        window(root, (0, 0, 50, 50), &plain);
        let hers = x11rb::connect(Some(&xvfb.display)).expect("hers").0;
        let glue = Glue {
            connection: hers,
            pet,
            main,
        };
        let pane = Bounds::new(point(px(10.), px(10.)), size(px(380.), px(280.)));
        glue.put(2.0, pane).expect("she is placed");
        let check = x11rb::connect(Some(&xvfb.display))
            .expect("the check connects")
            .0;
        let at = check
            .get_geometry(pet)
            .expect("the request")
            .reply()
            .expect("her place");
        let stands = (
            100 + ((10.0 + left(px(380.))) * 2.0).round() as i32,
            100 + ((10.0 - HEAD - ROOM) * 2.0).round() as i32,
        );
        assert_eq!((i32::from(at.x), i32::from(at.y)), stands);
        let order = check
            .query_tree(root)
            .expect("the request")
            .reply()
            .expect("the stack")
            .children;
        let place = |id| {
            order
                .iter()
                .position(|&child| child == id)
                .expect("on the root")
        };
        assert_eq!(
            place(pet),
            place(frame) + 1,
            "just above the frame: {order:?}"
        );
        let mapped = |check: &x11rb::rust_connection::RustConnection| {
            check
                .get_window_attributes(pet)
                .expect("the request")
                .reply()
                .expect("her state")
                .map_state
        };
        glue.shown(false).expect("hidden");
        assert_eq!(mapped(&check), MapState::UNMAPPED);
        glue.shown(true).expect("shown");
        assert_ne!(mapped(&check), MapState::UNMAPPED);
    }
}
