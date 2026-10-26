use super::process::{Cancellation, Event};

#[derive(Default)]
pub struct Briefing {
    pub visible: bool,
    pub generation: usize,
    pub text: String,
    pub error: Option<String>,
    pub attention: Vec<super::attention::Attention>,
    pub omitted: usize,
    pub anchor: super::placement::Anchor,
    pub running: Option<Cancellation>,
    pub task: Option<gpui::Task<()>>,
}

impl Briefing {
    pub fn cancel(&mut self) {
        self.running = None;
        self.task = None;
    }

    pub fn apply(&mut self, event: Event) -> bool {
        match event {
            Event::Reset => self.text.clear(),
            Event::Delta { text } => self.text.extend(
                text.chars()
                    .take(16_000_usize.saturating_sub(self.text.chars().count())),
            ),
            Event::Tool { name, session_id } => {
                log::debug!("butler: tool {name}, session {session_id:?}");
            }
            Event::Done { text, sources } => {
                log::info!("butler: report complete, {} sources", sources.len());
                self.text = text.chars().take(16_000).collect();
                self.running = None;
                return true;
            }
            Event::Error { text } => {
                log::info!("butler: report failed");
                self.error = Some(text);
                self.running = None;
                return true;
            }
        }
        false
    }
}
