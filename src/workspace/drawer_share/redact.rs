use std::sync::LazyLock;

use regex::Regex;

static RULES: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    [
        (r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?(?:-----END [A-Z ]*PRIVATE KEY-----|\z)", "[REDACTED PRIVATE KEY]"),
        (r"(?s)\A.*?-----END [A-Z ]*PRIVATE KEY-----", "[REDACTED PRIVATE KEY]"),
        (r"(?im)\b((?:set-cookie|cookie|authorization|proxy-authorization)\s*:\s*)[^\r\n]+", "${1}[REDACTED]"),
        (r"(?i)\b((?:bearer|basic)\s+)[a-z0-9+/_.=~\-]+", "${1}[REDACTED]"),
        (r#"(?i)([a-z][a-z0-9+.-]*://)[^\s/@]+@"#, "${1}[REDACTED]@"),
        (r#"(?i)((?:[\w-]*(?:api[_-]?key|authorization|cookie|token|secret|password|passwd|credential|signature|session[_-]?id|access[_-]?key|private[_-]?key)[\w-]*|pwd)["']?\s*(?:=|:|\s)\s*)(?:"(?:\\.|[^"\\\n])*"|'(?:\\.|[^'\\\n])*'|[^\s&,;"'}]+)"#, "${1}[REDACTED]"),
        (r"\b(?:sk-[A-Za-z0-9_-]{12,}|gh[pousr]_[A-Za-z0-9]{12,}|github_pat_[A-Za-z0-9_]{12,}|xox[baprs]-[A-Za-z0-9-]{10,}|AKIA[A-Z0-9]{16}|ASIA[A-Z0-9]{16}|AIza[A-Za-z0-9_-]{30,})\b", "[REDACTED]"),
        (r"\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b", "[REDACTED JWT]"),
        (r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b", "[REDACTED EMAIL]"),
        (r"/(?:Users|home)/[^/\s]+", "~"),
        (r"(?i)[a-z]:\\Users\\[^\\\s]+", "~"),
    ]
    .into_iter()
    .map(|(pattern, replacement)| (Regex::new(pattern).expect("valid redaction rule"), replacement))
    .collect()
});

pub(super) fn redact(text: &str) -> String {
    let mut text: String = text
        .chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
        .collect();
    for (pattern, replacement) in RULES.iter() {
        text = pattern.replace_all(&text, *replacement).into_owned();
    }
    text.trim().to_string()
}

#[cfg(test)]
mod tests;
