//! Localized status records shown when a session is hovered.

use super::plain;
use crate::term::{Kind, Record, Records, State};
use ely_gpui_component::i18n::I18n;
use gpui::SharedString;

pub(super) const LISTED: usize = 10;

/// What hover shows: a line for each record, ten at most, then a failed command's status.
pub fn hover(records: &Records, failed: Option<i32>, words: &I18n) -> Vec<SharedString> {
    let mut lines: Vec<SharedString> = records
        .iter()
        .take(LISTED)
        .map(|(id, record)| line(records, id, record, words).into())
        .collect();
    let more = records.iter().count().saturating_sub(LISTED);
    if more > 0 {
        lines.push(words.text("et.status.more", &[("count", &more.to_string())]));
    }
    if let Some(code) = failed {
        lines.push(words.text("et.status.exited", &[("code", &code.to_string())]));
    }
    lines
}

/// A record as hover and notifications say it: its state, then its app, title and message.
pub fn line(records: &Records, id: &str, record: &Record, words: &I18n) -> String {
    let parts = [
        records.app(id),
        record.title.as_deref(),
        record.msg.as_deref(),
    ];
    let said: Vec<String> = parts.into_iter().flatten().map(plain).collect();
    let state = label(record, words);
    if said.is_empty() {
        state.to_string()
    } else {
        let said = said.join(" · ");
        words
            .text("et.status.line", &[("state", &state), ("said", &said)])
            .to_string()
    }
}

/// A record's state, as hover names it.
fn label(record: &Record, words: &I18n) -> SharedString {
    let key = match (record.state, record.kind, record.progress) {
        (State::Idle, ..) => "et.status.idle",
        (State::Working, _, Some(value)) => {
            return words.text("et.status.working_at", &[("value", &value.to_string())]);
        }
        (State::Working, ..) => "et.status.working",
        (State::Blocked, Some(Kind::Permission), _) => "et.status.permission",
        (State::Blocked, Some(Kind::Question), _) => "et.status.question",
        (State::Blocked, Some(Kind::Auth), _) => "et.status.auth",
        (State::Blocked, None, _) => "et.status.waiting",
        (State::Done, ..) => "et.status.done",
        (State::Error, ..) => "et.status.error",
    };
    words.text(key, &[])
}
