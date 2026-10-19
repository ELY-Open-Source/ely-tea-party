//! Xvfb for the X11 tests.

use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
};

/// An X server of the test's own, gone with it.
pub struct Xvfb {
    child: Child,
    pub display: String,
}

impl Xvfb {
    pub fn start() -> Self {
        let mut child = Command::new("Xvfb")
            .args(["-displayfd", "1", "-nolisten", "tcp"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("Xvfb starts: the X11 tests need it");
        let mut number = String::new();
        let stdout = child.stdout.take().expect("Xvfb's stdout");
        BufReader::new(stdout)
            .read_line(&mut number)
            .expect("Xvfb names its display");
        let display = format!(":{}", number.trim());
        Self { child, display }
    }
}

impl Drop for Xvfb {
    fn drop(&mut self) {
        if let Err(error) = self.child.kill().and_then(|()| self.child.wait().map(drop)) {
            eprintln!("Xvfb {} did not stop: {error}", self.display);
        }
    }
}
