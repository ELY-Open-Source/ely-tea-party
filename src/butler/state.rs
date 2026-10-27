use super::process::{Cancellation, Event};

#[derive(Default)]
pub struct Briefing {
    pub visible: bool,
    pub generation: usize,
    pub text: String,
    pub error: Option<String>,
    pub target: Option<crate::sessions::LinkId>,
    pub expanded: Option<crate::sessions::LinkId>,
    pub cached: bool,
    pub tokens: Option<u64>,
    pub cache: super::cache::Cache,
    pub pending_key: Option<String>,
    pub anchor: super::placement::Anchor,
    pub running: Option<Cancellation>,
    pub task: Option<gpui::Task<()>>,
}

impl Briefing {
    pub fn cancel(&mut self) {
        self.running = None;
        self.task = None;
        self.pending_key = None;
    }

    pub fn apply(&mut self, event: Event) -> bool {
        match event {
            Event::Prepared { key } => self.pending_key = Some(key),
            Event::Cached { text } => {
                self.text = text;
                self.cached = true;
                self.running = None;
                self.pending_key = None;
                log::info!("butler: unchanged excerpt reused without a model request");
                return true;
            }
            Event::Reset => self.text.clear(),
            Event::Delta { text } => self.text.extend(
                text.chars()
                    .take(16_000_usize.saturating_sub(self.text.chars().count())),
            ),
            Event::Done {
                text,
                sources,
                tokens,
            } => {
                log::info!("butler: report complete, {} sources", sources.len());
                self.text = text.chars().take(16_000).collect();
                self.tokens = tokens;
                if let Some(key) = self.pending_key.take() {
                    self.cache
                        .insert(key, self.text.clone(), std::time::Instant::now());
                }
                self.running = None;
                return true;
            }
            Event::Error { text } => {
                log::info!("butler: report failed");
                self.error = Some(text);
                self.pending_key = None;
                self.running = None;
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn failed_and_cancelled_runs_do_not_enter_the_cache() {
        let mut state = Briefing::default();
        state.apply(Event::Prepared {
            key: "failed".into(),
        });
        state.apply(Event::Delta {
            text: "partial".into(),
        });
        state.apply(Event::Error {
            text: "failed".into(),
        });
        assert!(state.cache.get("failed", Instant::now()).is_none());
        state.apply(Event::Prepared {
            key: "cancelled".into(),
        });
        state.cancel();
        assert!(state.pending_key.is_none());
        assert!(state.cache.get("cancelled", Instant::now()).is_none());
    }

    #[test]
    fn successful_replies_are_reused_without_a_running_model() {
        let mut state = Briefing::default();
        state.apply(Event::Prepared { key: "same".into() });
        assert!(state.apply(Event::Done {
            text: "reported, not verified".into(),
            sources: vec!["1".into()],
            tokens: Some(123),
        }));
        let text = state.cache.get("same", Instant::now()).unwrap();
        state.apply(Event::Cached { text });
        assert!(state.cached);
        assert!(state.running.is_none());
        assert_eq!(state.text, "reported, not verified");
    }
}
