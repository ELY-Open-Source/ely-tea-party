use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

const COOLDOWN: Duration = Duration::from_secs(20 * 60);
const HOUR: Duration = Duration::from_secs(60 * 60);

pub struct Cadence {
    context: Option<String>,
    since: Option<Instant>,
    touched: Option<Instant>,
    tick: Option<Instant>,
    delay: Duration,
    probe: Option<Instant>,
}

impl Default for Cadence {
    fn default() -> Self {
        Self {
            context: None,
            since: None,
            touched: None,
            tick: None,
            delay: Duration::from_secs(300 + (uuid::Uuid::new_v4().as_u128() % 301) as u64),
            probe: None,
        }
    }
}

impl Cadence {
    pub fn reset(&mut self) {
        self.context = None;
        self.since = None;
        self.probe = None;
    }

    pub fn observe(
        &mut self,
        now: Instant,
        context: Option<String>,
        touched: Option<Instant>,
    ) -> bool {
        let interrupted = self
            .tick
            .is_some_and(|tick| now.saturating_duration_since(tick) > Duration::from_secs(5));
        self.tick = Some(now);
        if context.is_none() || context != self.context || touched != self.touched || interrupted {
            self.probe = None;
            self.since = context.as_ref().map(|_| now);
            self.context = context;
            self.touched = touched;
            return false;
        }
        let due = self
            .since
            .is_some_and(|since| now.saturating_duration_since(since) >= self.delay)
            && self
                .probe
                .is_none_or(|at| now.saturating_duration_since(at) >= Duration::from_secs(60));
        if due {
            self.probe = Some(now);
        }
        due
    }
}

#[derive(Default)]
pub struct Budget {
    attempts: VecDeque<Instant>,
    fingerprints: VecDeque<u64>,
}

impl gpui::Global for Budget {}

impl Budget {
    pub fn available(&self, now: Instant) -> bool {
        self.attempts
            .back()
            .is_none_or(|at| now.saturating_duration_since(*at) >= COOLDOWN)
            && self
                .attempts
                .iter()
                .filter(|at| now.saturating_duration_since(**at) < HOUR)
                .count()
                < 3
    }

    pub fn reserve(&mut self, now: Instant, fingerprint: u64) -> bool {
        if !self.available(now) || self.fingerprints.contains(&fingerprint) {
            return false;
        }
        self.attempts
            .retain(|at| now.saturating_duration_since(*at) < HOUR);
        self.attempts.push_back(now);
        self.fingerprints.push_back(fingerprint);
        while self.fingerprints.len() > 32 {
            self.fingerprints.pop_front();
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(
        cadence: &mut Cadence,
        start: Instant,
        seconds: u64,
        context: Option<&str>,
        touch: Option<Instant>,
    ) -> bool {
        let mut due = false;
        for second in 0..=seconds {
            due = cadence.observe(
                start + Duration::from_secs(second),
                context.map(str::to_string),
                touch,
            );
        }
        due
    }

    #[test]
    fn requires_continuous_attention_and_resets_for_input_switches_and_sleep() {
        let start = Instant::now();
        let mut cadence = Cadence {
            delay: Duration::from_secs(300),
            ..Default::default()
        };
        assert!(!tick(&mut cadence, start, 299, Some("project-a"), None));
        assert!(cadence.observe(
            start + Duration::from_secs(300),
            Some("project-a".into()),
            None
        ));
        let now = start + Duration::from_secs(301);
        assert!(!cadence.observe(now, Some("project-a".into()), Some(now)));
        assert!(!tick(&mut cadence, now, 299, Some("project-a"), Some(now)));
        assert!(!cadence.observe(
            now + Duration::from_secs(300),
            Some("project-b".into()),
            Some(now)
        ));
        assert!(!cadence.observe(now + Duration::from_secs(301), None, Some(now)));
        assert!(!tick(
            &mut cadence,
            now + Duration::from_secs(302),
            299,
            Some("project-b"),
            Some(now)
        ));
        assert!(!cadence.observe(
            now + Duration::from_secs(1000),
            Some("project-b".into()),
            Some(now)
        ));
    }

    #[test]
    fn failures_and_context_changes_do_not_bypass_cooldown_or_repeat_evidence() {
        let start = Instant::now();
        assert!((300..=600).contains(&Cadence::default().delay.as_secs()));
        let mut cadence = Budget::default();
        assert!(cadence.reserve(start, 1));
        assert!(!cadence.reserve(start + Duration::from_secs(1199), 2));
        assert!(!cadence.reserve(start + COOLDOWN, 1));
        assert!(cadence.reserve(start + COOLDOWN, 2));
        assert!(cadence.reserve(start + COOLDOWN * 2, 3));
        assert!(!cadence.reserve(start + HOUR - Duration::from_secs(1), 4));
        assert!(cadence.reserve(start + HOUR, 4));
    }
}
