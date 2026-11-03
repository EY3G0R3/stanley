//! A block that shows what a command prints: `script:~/bin/vpn-status`.
//!
//! A widget can be shared as a script this way, without touching Rust. The
//! command runs through `sh -c` every `EVERY`, never two at once, and never
//! holds up the bar: it is started on one tick and its output collected on a
//! later one. One killed for taking longer than `EVERY` keeps the last reading.
//!
//! The first line of output is the block. With a tab in it, the part before
//! the tab is the icon and the rest the extra text; without one, all of it is
//! the extra text. No output takes the block off the bar.

use super::{Block, Reading};
use crate::protocol::clean;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const EVERY: Duration = Duration::from_secs(10);

pub struct Script {
    command: String,
    running: Option<(Child, Instant)>,
    next: Instant,
    last: Reading,
}

impl Script {
    pub fn new(command: &str) -> Script {
        Script { command: command.to_string(), running: None, next: Instant::now(), last: Reading::default() }
    }
}

/// The reading for a command's output.
fn reading(output: &str) -> Reading {
    let line = output.lines().next().unwrap_or("");
    let (icon, extra) = match line.split_once('\t') {
        Some((icon, extra)) => (icon, extra),
        None => ("", line),
    };
    let part = |s: &str| (!s.trim().is_empty()).then(|| clean(s.trim()));
    Reading { icon: part(icon), extra: part(extra), ..Reading::default() }
}

impl Block for Script {
    /// Read every tick, which only checks on the command; it runs every
    /// `EVERY`.
    fn read(&mut self) -> Reading {
        if let Some((child, started)) = self.running.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    let mut output = String::new();
                    if let Some(mut out) = child.stdout.take() {
                        let _ = out.read_to_string(&mut output);
                    }
                    self.last = reading(&output);
                    self.running = None;
                }
                Ok(None) if started.elapsed() > EVERY => {
                    let _ = child.kill();
                    let _ = child.wait();
                    self.running = None;
                }
                Ok(None) => {}
                Err(_) => self.running = None,
            }
        }
        if self.running.is_none() && Instant::now() >= self.next {
            self.next = Instant::now() + EVERY;
            let child = Command::new("/bin/sh")
                .arg("-c")
                .arg(&self.command)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();
            match child {
                Ok(child) => self.running = Some((child, Instant::now())),
                Err(e) => eprintln!("stanley: script {}: {e}", self.command),
            }
        }
        self.last.clone()
    }
}

/// A bar-file reload drops the block; do not leave its command behind.
impl Drop for Script {
    fn drop(&mut self) {
        if let Some((child, _)) = self.running.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_to_parts() {
        assert_eq!(reading("I\tvpn up\nignored\n"), Reading { icon: Some("I".into()), extra: Some("vpn up".into()), ..Reading::default() });
        assert_eq!(reading("just text\n"), Reading { extra: Some("just text".into()), ..Reading::default() });
        assert_eq!(reading(""), Reading::default());
        assert_eq!(reading("a\x1fb"), Reading { extra: Some("a?b".into()), ..Reading::default() });
    }

    #[test]
    fn runs_without_blocking() {
        let mut s = Script::new("printf 'I\\tok\\n'");
        let start = Instant::now();
        assert_eq!(s.read(), Reading::default());
        assert!(start.elapsed() < Duration::from_millis(100));
        let deadline = Instant::now() + Duration::from_secs(5);
        while s.read().extra.is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(s.read().extra.as_deref(), Some("ok"));
    }
}
