//! Reading and writing an agent's config: each change backed up beside the file first.

use std::path::Path;

use serde_json::{Value, json};

/// The file's text, or None when it is missing.
pub fn read(path: &Path) -> anyhow::Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => anyhow::bail!("cannot read {}: {error}", path.display()),
    }
}

/// The JSON object in the file, `{}` when it is missing or empty.
pub fn read_json(path: &Path) -> anyhow::Result<(Option<String>, Value)> {
    let text = read(path)?;
    let value = match text.as_deref().map(str::trim) {
        None | Some("") => json!({}),
        Some(text) => serde_json::from_str(text)
            .map_err(|error| anyhow::anyhow!("{} is no JSON: {error}", path.display()))?,
    };
    anyhow::ensure!(value.is_object(), "{} holds no JSON object", path.display());
    Ok((text, value))
}

/// Writes `text` to `path` unless it already holds it, the old file kept as `NAME.ely-tea-party.bak`; returns whether it wrote.
pub fn write(path: &Path, old: Option<&str>, text: &str) -> anyhow::Result<bool> {
    if old == Some(text) {
        return Ok(false);
    }
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)
            .map_err(|error| anyhow::anyhow!("cannot make {}: {error}", folder.display()))?;
    }
    if let Some(old) = old {
        let name = path.file_name().map(|name| name.to_string_lossy());
        let backup =
            path.with_file_name(format!("{}.{}.bak", name.unwrap_or_default(), super::MARK));
        std::fs::write(&backup, old)
            .map_err(|error| anyhow::anyhow!("cannot back up to {}: {error}", backup.display()))?;
    }
    std::fs::write(path, text)
        .map_err(|error| anyhow::anyhow!("cannot write {}: {error}", path.display()))?;
    Ok(true)
}

/// Writes the JSON `value` as the file at `path`, pretty, unless it already holds it; returns whether it wrote.
pub fn write_json(path: &Path, old: Option<&str>, value: &Value) -> anyhow::Result<bool> {
    let text = serde_json::to_string_pretty(value)? + "\n";
    let same = old
        .and_then(|old| serde_json::from_str::<Value>(old).ok())
        .as_ref()
        == Some(value);
    if same {
        return Ok(false);
    }
    write(path, old, &text)
}

/// `text` as a TOML basic string.
pub fn toml_string(text: &str) -> String {
    let mut quoted = String::from('"');
    for c in text.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            c if c.is_control() => quoted.push_str(&format!("\\u{:04X}", c as u32)),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

/// `text` with the lines from `begin` to `end` holding `body`, the block added at the end when missing.
pub fn with_block(text: &str, (begin, end): (&str, &str), body: &str) -> String {
    let block = format!("{begin}\n{body}{end}\n");
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|line| line.trim() == begin);
    let stop = start.and_then(|start| {
        lines[start..]
            .iter()
            .position(|line| line.trim() == end)
            .map(|at| start + at)
    });
    match (start, stop) {
        (Some(start), Some(stop)) => {
            let mut out = lines[..start].join("\n");
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&block);
            for line in &lines[stop + 1..] {
                out.push_str(line);
                out.push('\n');
            }
            out
        }
        _ => {
            let mut out = text.trim_end().to_string();
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out + &block
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_is_added_once_then_replaced_in_place() {
        let marks = ("# b", "# e");
        let added = with_block("a = 1\n", marks, "x = 1\n");
        assert_eq!(added, "a = 1\n\n# b\nx = 1\n# e\n");
        let replaced = with_block(&(added + "z = 2\n"), marks, "x = 2\n");
        assert_eq!(replaced, "a = 1\n\n# b\nx = 2\n# e\nz = 2\n");
        assert_eq!(with_block("", marks, ""), "# b\n# e\n");
    }

    #[test]
    fn toml_strings_escape_quotes_and_backslashes() {
        assert_eq!(toml_string(r#"a "b" \c"#), r#""a \"b\" \\c""#);
    }
}
