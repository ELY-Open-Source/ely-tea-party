//! gpui builds its Mac platform only on the main thread, so this test brings its own main.

#[cfg(target_os = "macos")]
fn main() {
    use ely_tea_party::term::load_font;
    let system = gpui::TextSystem::new(gpui_platform::current_platform(true).text_system());
    assert!(load_font(&gpui::font("Menlo"), &system).is_ok());
    // A family missing, and one gpui passes over for lacking an `m`.
    for family in ["No Such Family", "Apple Color Emoji"] {
        let error = load_font(&gpui::font(family), &system).expect_err(family);
        assert!(error.to_string().contains(family), "{error}");
    }
    println!("fonts: a font gpui does not load is refused");
}

#[cfg(not(target_os = "macos"))]
fn main() {}
