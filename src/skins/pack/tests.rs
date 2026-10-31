use std::{collections::BTreeMap, io::Cursor, sync::Arc};

use image::{DynamicImage, ImageFormat, RgbaImage};
use serde_json::{Value, json};

use super::*;
use crate::skins::{Skin, files};

fn png() -> Arc<[u8]> {
    let image = RgbaImage::from_pixel(4, 4, image::Rgba([20, 120, 200, 255]));
    let mut out = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut out, ImageFormat::Png)
        .unwrap();
    out.into_inner().into()
}

fn meta() -> Meta {
    Meta {
        id: Some("tester.blue".into()),
        name: "Blue".into(),
        author: "Tester".into(),
        description: Some("Two lines,\nboth shown.".into()),
        license: Some("CC-BY-4.0 OR MIT".into()),
        homepage: Some("https://example.com/blue".into()),
    }
}

/// A valid v2 document for `slot` as JSON, to bend in each test.
fn v2(slot: Slot) -> Value {
    let skin = Skin::new(
        slot,
        BTreeMap::from([("idle".into(), png()), ("thinking".into(), png())]),
        Layout::default(),
    )
    .unwrap()
    .with_meta(Some(meta()));
    serde_json::from_slice(&files::document(slot, &skin).unwrap()).unwrap()
}

fn bytes(document: &Value) -> Vec<u8> {
    serde_json::to_vec(document).unwrap()
}

fn refused(document: &Value, slot: Slot) -> String {
    match read(&bytes(document), slot) {
        Ok(_) => panic!("accepted {document}"),
        Err(error) => format!("{error:#}"),
    }
}

#[test]
fn credits_and_images_round_trip_in_v2_and_a_skin_without_credits_stays_v1() {
    let document = v2(Slot::Claude);
    assert_eq!(document["format"], FORMAT);
    assert_eq!(document["version"], 2);
    let keys: Vec<_> = document.as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        keys,
        ["format", "version", "target", "meta", "layout", "images"]
    );
    let read = read(&bytes(&document), Slot::Claude).unwrap();
    assert_eq!(read.meta, Some(meta()));
    assert_eq!(read.images.keys().collect::<Vec<_>>(), ["idle", "thinking"]);
    assert!(read.skipped.is_empty());
    let plain = Skin::new(
        Slot::Claude,
        BTreeMap::from([("idle".into(), png())]),
        Layout::default(),
    )
    .unwrap();
    let old: Value =
        serde_json::from_slice(&files::document(Slot::Claude, &plain).unwrap()).unwrap();
    assert_eq!(old["version"], 1);
    assert!(old.get("format").is_none() && old.get("meta").is_none());
}

#[test]
fn a_later_build_s_fields_and_actions_are_skipped_not_refused() {
    let mut document = v2(Slot::Claude);
    document["tags"] = json!(["ocean"]);
    document["meta"]["avatar"] = json!("https://example.com/a.png");
    document["layout"]["rotation"] = json!(5);
    document["images"]["dancing"] = document["images"]["idle"].clone();
    let read = read(&bytes(&document), Slot::Claude).unwrap();
    assert_eq!(read.skipped, ["dancing"]);
    assert_eq!(read.images.len(), 2);
    let mut old = document.clone();
    old["version"] = json!(1);
    assert!(read_v1_refuses(&old));
}

fn read_v1_refuses(document: &Value) -> bool {
    read(&bytes(document), Slot::Claude).is_err()
}

#[test]
fn any_agent_takes_any_agent_s_pack_even_one_this_build_does_not_know() {
    let mut document = v2(Slot::Claude);
    document["target"] = json!("codex");
    assert!(read(&bytes(&document), Slot::Gpt).is_ok());
    assert!(refused(&document, Slot::Whale).contains("for codex"));
    let mascot = v2(Slot::Whale);
    assert!(refused(&mascot, Slot::Claude).contains("for whale"));
    document["target"] = json!("../codex");
    assert!(refused(&document, Slot::Claude).contains("invalid target"));
}

#[test]
fn newer_versions_missing_format_and_repeated_actions_are_refused() {
    let mut document = v2(Slot::Claude);
    document["version"] = json!(3);
    assert!(refused(&document, Slot::Claude).contains("newer Ely Tea Party"));
    let mut document = v2(Slot::Claude);
    document["format"] = json!("other");
    assert!(refused(&document, Slot::Claude).contains("format"));
    document.as_object_mut().unwrap().remove("format");
    refused(&document, Slot::Claude);
    let mut document = v2(Slot::Claude);
    document.as_object_mut().unwrap().remove("meta");
    refused(&document, Slot::Claude);
    let text = String::from_utf8(bytes(&v2(Slot::Claude))).unwrap();
    let idle = v2(Slot::Claude)["images"]["idle"]
        .as_str()
        .unwrap()
        .to_string();
    let twice = text.replacen(
        "\"images\":{",
        &format!("\"images\":{{\"idle\":\"{idle}\","),
        1,
    );
    let error = read(twice.as_bytes(), Slot::Claude).err().unwrap();
    assert!(format!("{error:#}").contains("duplicate action idle"));
}

#[test]
fn action_names_must_be_plain_and_few() {
    for name in ["../outside", "Idle", "", "a b"] {
        let mut document = v2(Slot::Claude);
        document["images"][name] = document["images"]["idle"].clone();
        assert!(
            refused(&document, Slot::Claude).contains("invalid action name"),
            "{name:?}"
        );
    }
    let mut document = v2(Slot::Claude);
    for i in 0..MOST_ENTRIES {
        document["images"][format!("extra-{i}")] = json!("");
    }
    assert!(refused(&document, Slot::Claude).contains("too many actions"));
}

#[test]
fn credits_refuse_blank_long_hidden_and_unsafe_text() {
    let cases: [(&str, Value); 11] = [
        ("name", json!("")),
        ("name", json!(" Blue")),
        ("name", json!("x".repeat(65))),
        ("author", json!("Tes\u{0007}ter")),
        ("author", json!("evil\u{202E}txt.exe")),
        ("description", json!("tab\there")),
        ("license", json!("CC-BY-4.0; rm -rf")),
        ("homepage", json!("http://example.com")),
        ("homepage", json!("https://exa mple.com")),
        ("id", json!("Blue Pack")),
        ("id", json!("-blue")),
    ];
    for (field, value) in cases {
        let mut document = v2(Slot::Claude);
        document["meta"][field] = value.clone();
        assert!(
            refused(&document, Slot::Claude).contains(&format!("meta.{field}")),
            "{field} = {value}"
        );
    }
    let mut document = v2(Slot::Claude);
    document["meta"]["name"] = json!("蓝色鲸鱼 🐋");
    assert!(read(&bytes(&document), Slot::Claude).is_ok());
}

#[test]
fn writing_refuses_credits_a_reader_would_refuse() {
    let skin = Skin::new(
        Slot::Claude,
        BTreeMap::from([("idle".into(), png())]),
        Layout::default(),
    )
    .unwrap()
    .with_meta(Some(Meta {
        author: String::new(),
        ..meta()
    }));
    assert!(files::document(Slot::Claude, &skin).is_err());
}
