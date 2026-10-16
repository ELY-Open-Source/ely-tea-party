use std::sync::{Mutex, Once};

static LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Keeps every log line so tests can assert what was logged.
struct Capture;

impl log::Log for Capture {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Debug && metadata.target().starts_with("ely_tea_party")
    }

    fn log(&self, record: &log::Record) {
        let line = record.args().to_string();
        eprintln!("{} {line}", record.level());
        LINES.lock().expect("the log lines").push(line);
    }

    fn flush(&self) {}
}

pub fn init() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        log::set_logger(&Capture).expect("the only logger in tests");
        log::set_max_level(log::LevelFilter::Debug);
    });
}

pub fn logged(line: &str) -> bool {
    LINES
        .lock()
        .expect("the log lines")
        .iter()
        .any(|kept| kept == line)
}
