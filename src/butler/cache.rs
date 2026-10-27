use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use crate::term::snapshot::Snapshot;

const TTL: Duration = Duration::from_secs(300);
const KEPT: usize = 8;
pub const EXCERPT_BYTES: usize = 6_000;
pub const EXCERPT_LINES: usize = 60;

#[derive(Clone, Default)]
pub struct Cache(VecDeque<(String, String, Instant)>);

impl Cache {
    pub fn get(&self, key: &str, now: Instant) -> Option<String> {
        self.0
            .iter()
            .find(|(k, _, at)| k == key && now.saturating_duration_since(*at) < TTL)
            .map(|(_, text, _)| text.clone())
    }

    pub fn insert(&mut self, key: String, text: String, now: Instant) {
        self.0
            .retain(|(k, _, at)| k != &key && now.saturating_duration_since(*at) < TTL);
        self.0.push_back((key, text, now));
        while self.0.len() > KEPT {
            self.0.pop_front();
        }
    }
}

pub fn compact(snapshot: &mut Snapshot) {
    while snapshot
        .lines
        .last()
        .is_some_and(|line| line.trim().is_empty())
    {
        snapshot.lines.pop();
    }
    let mut bytes = 0;
    let mut lines = Vec::new();
    for line in snapshot.lines.iter().rev().take(EXCERPT_LINES) {
        let remaining = EXCERPT_BYTES.saturating_sub(bytes);
        if remaining <= 1 {
            break;
        }
        let mut start = line.len().saturating_sub(remaining - 1);
        while !line.is_char_boundary(start) {
            start += 1;
        }
        let line = line[start..].to_string();
        bytes += line.len() + 1;
        lines.push(line);
    }
    lines.reverse();
    snapshot.truncated |= lines != snapshot.lines;
    snapshot.lines = lines;
}

pub fn key(request: &super::process::Request) -> String {
    let snapshot = &request.snapshots[0];
    serde_json::json!([
        request.provider,
        request.model,
        request.language,
        request.persona,
        request.status,
        snapshot.id,
        snapshot.url,
        snapshot.title,
        snapshot.folder,
        snapshot.alternate_screen,
        snapshot.truncated,
        snapshot.lines
    ])
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Snapshot {
        Snapshot {
            id: "1".into(),
            url: "ely://session/test".into(),
            title: "A".into(),
            folder: "/tmp".into(),
            captured_at: "now".into(),
            alternate_screen: false,
            truncated: false,
            lines: vec!["鲸".repeat(4_000), "latest evidence".into(), "".into()],
        }
    }

    #[test]
    fn compact_keeps_newest_evidence_and_bounds_unicode_bytes() {
        let mut s = snapshot();
        compact(&mut s);
        assert_eq!(s.lines.last().unwrap(), "latest evidence");
        assert!(s.lines.iter().map(|line| line.len() + 1).sum::<usize>() <= EXCERPT_BYTES);
        assert!(s.truncated);
        s.lines = (0..100).map(|n| n.to_string()).collect();
        compact(&mut s);
        assert_eq!(s.lines.len(), EXCERPT_LINES);
        assert_eq!(s.lines.last().unwrap(), "99");
    }

    #[test]
    fn cache_is_bounded_and_expires_without_extending_on_read() {
        let mut cache = Cache::default();
        let now = Instant::now();
        for n in 0..9 {
            cache.insert(n.to_string(), "reply".into(), now);
        }
        assert!(cache.get("0", now).is_none());
        assert_eq!(cache.get("1", now).as_deref(), Some("reply"));
        assert!(cache.get("1", now + TTL).is_none());
    }

    #[test]
    fn cache_ignores_capture_clock_but_tracks_evidence_model_and_persona() {
        let mut r = super::super::process::Request {
            version: 1,
            provider: "deepseek".into(),
            model: "model".into(),
            api_key: "secret-not-in-key".into(),
            language: "zh-CN".into(),
            persona: "".into(),
            status: super::super::tasks::Status::Idle,
            snapshots: vec![snapshot()],
        };
        let original = key(&r);
        assert!(!original.contains("secret-not-in-key"));
        r.snapshots[0].captured_at = "later".into();
        assert_eq!(key(&r), original);
        r.snapshots[0].lines.push("changed".into());
        assert_ne!(key(&r), original);
        let changed = key(&r);
        r.model = "other".into();
        assert_ne!(key(&r), changed);
        let changed = key(&r);
        r.persona = "new tone".into();
        assert_ne!(key(&r), changed);
    }
}
