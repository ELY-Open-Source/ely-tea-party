//! The `.elyskin` document: v1 as 0.3.12 wrote it, v2 with `format` and `meta`; `docs/elyskin.md` is its spec.

use std::{collections::BTreeMap, fmt};

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor},
};

use super::{Layout, Slot};

/// The newest version this build reads and the one it writes.
pub const VERSION: u32 = 2;
pub const FORMAT: &str = "elyskin";
/// Entries an `images` map may hold, known actions or not.
const MOST_ENTRIES: usize = 64;

/// A pack's credits, shown before it is installed and kept with the skin.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
}

impl Meta {
    pub fn validate(&self) -> anyhow::Result<()> {
        line("name", &self.name, 64)?;
        line("author", &self.author, 64)?;
        if let Some(id) = &self.id {
            let edge =
                |c: Option<char>| c.is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
            anyhow::ensure!(
                (3..=64).contains(&id.len())
                    && id.chars().all(|c| c.is_ascii_lowercase()
                        || c.is_ascii_digit()
                        || c == '.'
                        || c == '-')
                    && edge(id.chars().next())
                    && edge(id.chars().last()),
                "skin: meta.id must be 3 to 64 of a-z, 0-9, '.' and '-', starting and ending with a letter or digit"
            );
        }
        if let Some(description) = &self.description {
            text("description", description, 500, true)?;
        }
        if let Some(license) = &self.license {
            line("license", license, 64)?;
            anyhow::ensure!(
                license
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || " .+-()".contains(c)),
                "skin: meta.license must be an SPDX expression"
            );
        }
        if let Some(homepage) = &self.homepage {
            anyhow::ensure!(
                homepage.len() <= 200
                    && homepage.len() > "https://".len()
                    && homepage.starts_with("https://")
                    && !homepage
                        .chars()
                        .any(|c| c.is_whitespace() || c.is_control()),
                "skin: meta.homepage must be an https URL of at most 200 bytes"
            );
        }
        Ok(())
    }
}

/// One line of text: trimmed, not empty, without controls.
fn line(field: &str, value: &str, most: usize) -> anyhow::Result<()> {
    anyhow::ensure!(
        !value.trim().is_empty() && value.trim() == value,
        "skin: meta.{field} must be non-empty and trimmed"
    );
    text(field, value, most, false)
}

/// Text of at most `most` characters, free of controls and bidirectional overrides.
fn text(field: &str, value: &str, most: usize, lines: bool) -> anyhow::Result<()> {
    anyhow::ensure!(
        value.chars().count() <= most,
        "skin: meta.{field} exceeds {most} characters"
    );
    let bidi = |c: char| matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}');
    anyhow::ensure!(
        !value
            .chars()
            .any(|c| (c.is_control() && !(lines && c == '\n')) || bidi(c)),
        "skin: meta.{field} holds a control or bidirectional character"
    );
    Ok(())
}

/// A document read: its images by action, and what this build skipped.
pub struct Read {
    pub meta: Option<Meta>,
    pub layout: Layout,
    pub images: BTreeMap<String, String>,
    pub skipped: Vec<String>,
}

#[derive(Deserialize)]
struct Header {
    version: u32,
}

/// v1, as 0.3.12 to 0.3.16 wrote and read it: nothing unknown.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct V1 {
    #[allow(dead_code)]
    version: u32,
    target: Slot,
    #[serde(default)]
    layout: Layout,
    #[serde(deserialize_with = "unique")]
    images: BTreeMap<String, String>,
}

/// v2: unknown fields pass, so later builds may add them.
#[derive(Deserialize)]
struct V2 {
    format: String,
    target: String,
    meta: Meta,
    #[serde(default)]
    layout: Wire,
    #[serde(deserialize_with = "unique")]
    images: BTreeMap<String, String>,
}

/// v2's layout, unknown fields passing.
#[derive(Deserialize)]
#[serde(default)]
struct Wire {
    scale: f32,
    x: f32,
    y: f32,
}

impl Default for Wire {
    fn default() -> Self {
        let Layout { scale, x, y } = Layout::default();
        Self { scale, x, y }
    }
}

#[derive(Serialize)]
struct Out<'a> {
    format: &'static str,
    version: u32,
    target: &'static str,
    meta: &'a Meta,
    layout: Layout,
    images: BTreeMap<&'a str, String>,
}

#[derive(Serialize)]
struct OutV1<'a> {
    version: u32,
    target: Slot,
    layout: Layout,
    images: BTreeMap<&'a str, String>,
}

/// Reads a document for `slot`: a mascot pack for the mascot, any agent's for an agent.
pub fn read(bytes: &[u8], slot: Slot) -> anyhow::Result<Read> {
    let header: Header = serde_json::from_slice(bytes)
        .map_err(|e| anyhow::anyhow!("skin: not a character pack: {e}"))?;
    let (target, meta, layout, images) = match header.version {
        1 => {
            let pack: V1 = serde_json::from_slice(bytes)
                .map_err(|e| anyhow::anyhow!("skin: invalid character pack: {e}"))?;
            (pack.target.id().to_string(), None, pack.layout, pack.images)
        }
        2 => {
            let pack: V2 = serde_json::from_slice(bytes)
                .map_err(|e| anyhow::anyhow!("skin: invalid character pack: {e}"))?;
            anyhow::ensure!(pack.format == FORMAT, "skin: format must be \"{FORMAT}\"");
            pack.meta.validate()?;
            let Wire { scale, x, y } = pack.layout;
            (
                pack.target,
                Some(pack.meta),
                Layout { scale, x, y },
                pack.images,
            )
        }
        version if version > VERSION => anyhow::bail!(
            "skin: this pack needs a newer Ely Tea Party (pack version {version}, this app reads up to {VERSION})"
        ),
        version => anyhow::bail!("skin: unsupported pack version {version}"),
    };
    anyhow::ensure!(
        target.len() <= 32
            && target.starts_with(|c: char| c.is_ascii_lowercase())
            && target
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'),
        "skin: invalid target"
    );
    anyhow::ensure!(
        (target == Slot::Whale.id()) == (slot == Slot::Whale),
        "skin: this pack is for {target}, select that character first"
    );
    anyhow::ensure!(images.len() <= MOST_ENTRIES, "skin: too many actions");
    let mut kept = BTreeMap::new();
    let mut skipped = Vec::new();
    for (pose, encoded) in images {
        anyhow::ensure!(
            !pose.is_empty()
                && pose.len() <= 32
                && pose
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'),
            "skin: invalid action name {pose:?}"
        );
        if slot.poses().contains(&pose.as_str()) {
            kept.insert(pose, encoded);
        } else if header.version == 1 {
            anyhow::bail!("skin: unknown action {pose}");
        } else {
            skipped.push(pose);
        }
    }
    if !skipped.is_empty() {
        log::warn!(
            "skin: actions this build does not know were skipped: {}",
            skipped.join(", ")
        );
    }
    Ok(Read {
        meta,
        layout,
        images: kept,
        skipped,
    })
}

/// The document for `slot`: v2 with `meta`, else v1, which 0.3.12 and later read.
pub fn write(
    slot: Slot,
    meta: Option<&Meta>,
    layout: Layout,
    images: BTreeMap<&str, String>,
) -> anyhow::Result<Vec<u8>> {
    Ok(match meta {
        Some(meta) => {
            meta.validate()?;
            serde_json::to_vec(&Out {
                format: FORMAT,
                version: VERSION,
                target: slot.id(),
                meta,
                layout,
                images,
            })?
        }
        None => serde_json::to_vec(&OutV1 {
            version: 1,
            target: slot,
            layout,
            images,
        })?,
    })
}

/// A JSON object whose keys appear once: a repeated action is refused, not overwritten.
fn unique<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    struct Unique;
    impl<'de> Visitor<'de> for Unique {
        type Value = BTreeMap<String, String>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("an object of base64 strings")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut out = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, String>()? {
                if out.contains_key(&key) {
                    return Err(serde::de::Error::custom(format!("duplicate action {key}")));
                }
                if out.len() >= MOST_ENTRIES {
                    return Err(serde::de::Error::custom("too many actions"));
                }
                out.insert(key, value);
            }
            Ok(out)
        }
    }
    deserializer.deserialize_map(Unique)
}

#[cfg(test)]
mod tests;
