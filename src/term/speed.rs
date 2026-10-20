//! Codex 0.163 emits this styled confirmation when a session's service tier changes.

use crate::agents::speed::Speed;

const PREFIX: &[u8] = "\x1b[K\x1b[2m• \x1b[22mService tier set to ".as_bytes();
const END: &[u8] = b"\x1b[0m";
const LIMIT: usize = 128;

#[derive(Default)]
pub(super) struct Scanner {
    pending: Vec<u8>,
}

impl Scanner {
    pub fn feed(&mut self, bytes: &[u8], mut found: impl FnMut(Speed)) {
        for &byte in bytes {
            self.pending.push(byte);
            if self.pending.len() <= PREFIX.len() {
                if !PREFIX.starts_with(&self.pending) {
                    self.pending.clear();
                    if byte == PREFIX[0] {
                        self.pending.push(byte);
                    }
                }
                continue;
            }
            if self.pending.ends_with(END) {
                let tier = &self.pending[PREFIX.len()..self.pending.len() - END.len()];
                let speed = match tier {
                    b"fast" | b"priority" => Speed::Fast,
                    b"ultrafast" => Speed::Ultrafast,
                    b"default" => Speed::Standard,
                    _ => Speed::Unknown,
                };
                found(speed);
                self.pending.clear();
            } else if self.pending.len() >= LIMIT || byte == b'\n' {
                self.pending.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmations_survive_every_read_boundary_and_keep_order() {
        let mut bytes = PREFIX.to_vec();
        bytes.extend(b"ultrafast\x1b[0m");
        bytes.extend(PREFIX);
        bytes.extend(b"default\x1b[0m");
        for split in 0..bytes.len() {
            let mut scanner = Scanner::default();
            let mut found = Vec::new();
            scanner.feed(&bytes[..split], |speed| found.push(speed));
            scanner.feed(&bytes[split..], |speed| found.push(speed));
            assert_eq!(found, [Speed::Ultrafast, Speed::Standard]);
        }
    }

    #[test]
    fn prose_is_not_a_speed_signal() {
        let mut scanner = Scanner::default();
        scanner.feed(b"Service tier set to ultrafast\nFast on\n", |_| {
            panic!("prose")
        });
        scanner.feed(PREFIX, |_| {});
        scanner.feed(&[b'x'; 200], |_| {});
        assert!(scanner.pending.len() <= LIMIT);
    }
}
