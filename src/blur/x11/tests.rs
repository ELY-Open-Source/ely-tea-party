use x11rb::{
    COPY_DEPTH_FROM_PARENT,
    connection::Connection as _,
    protocol::xproto::{CreateWindowAux, WindowClass},
};

use super::*;
use crate::testx11::Xvfb;

#[test]
fn the_window_names_its_pane_while_blurred() {
    crate::testlog::init();
    let xvfb = Xvfb::start();
    let (connection, screen) = x11rb::connect(Some(&xvfb.display)).expect("the test connects");
    let root = connection.setup().roots[screen].root;
    let window = connection.generate_id().expect("a window id");
    let aux = CreateWindowAux::new();
    let class = WindowClass::INPUT_OUTPUT;
    (connection.create_window(
        COPY_DEPTH_FROM_PARENT,
        window,
        root,
        0,
        0,
        640,
        400,
        0,
        class,
        0,
        &aux,
    ))
    .expect("the request")
    .check()
    .expect("a window");
    let mut effect = Effect::new(Some(&xvfb.display), window).expect("an effect");
    let atom = effect.atom;
    let worn = || {
        let property = (connection.get_property(false, window, atom, AtomEnum::ANY, 0, 16))
            .expect("the request")
            .reply()
            .expect("the property");
        let cardinal = property.type_ == u32::from(AtomEnum::CARDINAL);
        let values = property.value32().map(Iterator::collect::<Vec<_>>);
        (property.type_ != 0).then_some((cardinal, values))
    };
    assert_eq!(worn(), None, "none at first");
    effect.blur(Some([20, 20, 1200, 760]));
    assert_eq!(worn(), Some((true, Some(vec![20, 20, 1200, 760]))));
    effect.blur(Some([0, 0, 1240, 800]));
    assert_eq!(worn(), Some((true, Some(vec![0, 0, 1240, 800]))));
    effect.blur(None);
    assert_eq!(worn(), None, "removed");
    assert!(crate::testlog::logged(&format!(
        "blur: {BLUR_BEHIND} on X11 window {window}; KWin blurs behind it, other compositors pass it by"
    )));
}
