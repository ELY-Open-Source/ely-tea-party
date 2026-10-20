//! Codex's title exposes Fast without changing its selected service tier.

use toml_edit::{Array, DocumentMut, Item, Table, value};

pub fn install(config: &str) -> anyhow::Result<String> {
    let mut doc: DocumentMut = config.parse()?;
    if doc.get("tui").is_none() {
        doc["tui"] = Item::Table(Table::new());
    }
    anyhow::ensure!(
        doc["tui"].is_table_like(),
        "install: Codex tui is not a table"
    );
    let empty = Item::None;
    let title = doc["tui"].get("terminal_title").unwrap_or(&empty);
    let mut fields = match title.as_array() {
        Some(fields) if fields.is_empty() => return Ok(config.to_string()),
        Some(fields) => fields.clone(),
        None if title.is_none() => ["spinner", "project"].into_iter().collect::<Array>(),
        None => anyhow::bail!("install: Codex tui.terminal_title is not an array"),
    };
    anyhow::ensure!(
        fields.iter().all(|item| item.as_str().is_some()),
        "install: Codex title identifiers must be strings"
    );
    let before: Vec<_> = fields
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    for index in (0..fields.len()).rev() {
        if matches!(
            fields.get(index).and_then(|v| v.as_str()),
            Some("app-name" | "fast-mode")
        ) {
            fields.remove(index);
        }
    }
    fields.insert(0, "app-name");
    fields.push("fast-mode");
    if fields
        .iter()
        .filter_map(|v| v.as_str())
        .eq(before.iter().map(String::as_str))
    {
        return Ok(config.to_string());
    }
    doc["tui"]["terminal_title"] = value(fields);
    Ok(doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_preserve_preferences_and_install_once() {
        for config in [
            "# owner\nmodel = \"example\"\n",
            "tui.terminal_title = [\"project\", \"model\"]\n",
            "[tui]\nterminal_title = [\n\"project\", # project\n\"fast-mode\",\n]\n",
            "tui = { terminal_title = [\"model\"] }\n",
        ] {
            let installed = install(config).unwrap();
            let parsed: DocumentMut = installed.parse().unwrap();
            let fields = parsed["tui"]["terminal_title"].as_array().unwrap();
            assert_eq!(fields.get(0).and_then(|v| v.as_str()), Some("app-name"));
            assert_eq!(
                fields.get(fields.len() - 1).and_then(|v| v.as_str()),
                Some("fast-mode")
            );
            assert_eq!(install(&installed).unwrap(), installed);
        }
        assert_eq!(
            install("[tui]\nterminal_title = []\n").unwrap(),
            "[tui]\nterminal_title = []\n"
        );
        assert!(install("tui = 4").is_err());
    }
}
