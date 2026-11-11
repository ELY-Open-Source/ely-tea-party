use std::{cmp::Reverse, ops::Range};

use gpui::{App, Global};

#[derive(Default)]
pub(super) struct Recent(Vec<u64>);

impl Global for Recent {}

impl Recent {
    pub(super) fn rank(&self, id: u64) -> usize {
        self.0
            .iter()
            .position(|&seen| seen == id)
            .unwrap_or(usize::MAX)
    }

    fn visit(&mut self, id: u64) {
        self.0.retain(|&seen| seen != id);
        self.0.insert(0, id);
        self.0.truncate(512);
    }
}

pub(crate) fn visit(id: u64, cx: &mut App) {
    cx.default_global::<Recent>().visit(id);
}

#[derive(Debug)]
pub(super) struct Match {
    pub score: i64,
    pub hits: [Vec<Range<usize>>; 3],
}

fn fuzzy(term: &str, value: &str) -> Option<(i64, Vec<Range<usize>>)> {
    let chars: Vec<_> = value.char_indices().collect();
    let mut next = 0;
    let mut score = 0;
    let mut hits = Vec::new();
    for needle in term.chars() {
        let at =
            (next..chars.len()).find(|&at| chars[at].1.to_lowercase().eq(needle.to_lowercase()))?;
        score += if at == next { 12 } else { 2 };
        if at == 0 || !chars[at - 1].1.is_alphanumeric() {
            score += 8;
        }
        score -= (at - next).min(16) as i64;
        let (byte, c) = chars[at];
        hits.push(byte..byte + c.len_utf8());
        next = at + 1;
    }
    if value.to_lowercase() == term.to_lowercase() {
        score += 64;
    }
    Some((score, hits))
}

pub(super) fn matches(query: &str, fields: [&str; 3]) -> Option<Match> {
    let mut found = Match {
        score: 0,
        hits: Default::default(),
    };
    for term in query.split_whitespace() {
        let (score, field, hits) = fields
            .iter()
            .enumerate()
            .filter_map(|(field, value)| {
                fuzzy(term, value).map(|(score, hits)| (score + [40, 0, 16][field], field, hits))
            })
            .max_by_key(|(score, _, _)| *score)?;
        found.score += score;
        found.hits[field].extend(hits);
    }
    for hits in &mut found.hits {
        hits.sort_by_key(|hit| hit.start);
        hits.dedup();
    }
    Some(found)
}

pub(super) fn order(
    empty: bool,
    current: bool,
    score: i64,
    recent: usize,
    state: u8,
    id: u64,
) -> (bool, Reverse<i64>, usize, u8, u64) {
    (empty && current, Reverse(score), recent, state, id)
}

pub(super) fn selected(ids: &[u64], previous: Option<u64>) -> Option<u64> {
    previous
        .filter(|id| ids.contains(id))
        .or_else(|| ids.first().copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_can_match_different_fields_and_all_are_required() {
        assert!(
            matches(
                "auth claude api",
                ["Fix authentication", "/work/api", "Claude Code"]
            )
            .is_some()
        );
        assert!(
            matches(
                "auth gemini",
                ["Fix authentication", "/work/api", "Claude Code"]
            )
            .is_none()
        );
        assert!(matches("   ", ["", "", ""]).is_some());
    }

    #[test]
    fn unicode_hits_are_valid_byte_ranges_and_repeated_words_do_not_overlap() {
        let found = matches("修复 修复", ["修复登录", "", ""]).unwrap();
        assert_eq!(found.hits[0], [0..3, 3..6]);
        for hit in found.hits[0].clone() {
            assert!("修复登录".get(hit).is_some());
        }
        assert!(matches("cDX", ["Codex", "", ""]).is_some());
        assert!(matches("é", ["Équipe", "", ""]).is_some());
    }

    #[test]
    fn name_and_exact_matches_rank_above_path_and_loose_matches() {
        let score = |fields| matches("api", fields).unwrap().score;
        assert!(score(["api", "", ""]) > score(["", "/work/api", ""]));
        assert!(score(["api", "", ""]) > score(["a long pending issue", "", ""]));
    }

    #[test]
    fn empty_search_uses_recency_then_status_and_places_current_last() {
        let mut rows = [
            order(true, true, 0, 0, 0, 1),
            order(true, false, 0, 1, 5, 2),
            order(true, false, 0, usize::MAX, 3, 3),
            order(true, false, 0, usize::MAX, 1, 4),
        ];
        rows.sort();
        assert_eq!(rows.map(|row| row.4), [2, 4, 3, 1]);
        assert!(order(false, true, 100, 0, 0, 1) < order(false, false, 10, 1, 0, 2));
    }

    #[test]
    fn recency_and_selection_follow_ids_after_reordering_or_removal() {
        let mut recent = Recent::default();
        for id in [1, 2, 3, 2] {
            recent.visit(id);
        }
        assert_eq!(recent.0, [2, 3, 1]);
        assert_eq!(selected(&[3, 2, 1], Some(2)), Some(2));
        assert_eq!(selected(&[3, 1], Some(2)), Some(3));
        assert_eq!(selected(&[], Some(2)), None);
    }
}
