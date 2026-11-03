//! Clicks from dwl, on a FIFO.
//!
//! dwl knows only where the pointer landed; stanley keeps the mapping from
//! block to command, so a new clickable widget is a line in the bar file and
//! nothing has to be taught to the compositor.
//!
//! Two FIFOs while dwl builds older than the position message are around:
//! `stanley-click` carries six bytes (block, button, then x and the monitor's
//! width as little-endian u16), and `slstatus-click` the two that slstatus
//! reads. A dwl that knows the first uses it whenever stanley holds it open.

use std::ffi::CString;
use std::os::fd::RawFd;
use std::process::{Child, Command, Stdio};

/// One click. `at` is the pointer's x and the monitor's width, in
/// monitor-local logical pixels, when dwl sent them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Click {
    pub block: u8,
    pub button: u8,
    pub at: Option<(u16, u16)>,
}

impl Click {
    fn parse(record: &[u8]) -> Click {
        let at = (record.len() == 6).then(|| {
            (u16::from_le_bytes([record[2], record[3]]), u16::from_le_bytes([record[4], record[5]]))
        });
        Click { block: record[0], button: record[1], at }
    }
}

/// `$XDG_RUNTIME_DIR/stanley-click`, and `$SLSTATUS_CLICK_FIFO`, else
/// `$XDG_RUNTIME_DIR/slstatus-click`, with the size of their records.
fn paths() -> Vec<(String, usize)> {
    let dir = std::env::var("XDG_RUNTIME_DIR").ok();
    let mut paths = Vec::new();
    if let Some(dir) = &dir {
        paths.push((format!("{dir}/stanley-click"), 6));
    }
    if let Some(p) = std::env::var("SLSTATUS_CLICK_FIFO").ok().or(dir.map(|d| format!("{d}/slstatus-click"))) {
        paths.push((p, 2));
    }
    paths
}

/// Open a FIFO, creating it if needed.
fn open(path: &str) -> Option<RawFd> {
    let c = CString::new(path).ok()?;
    if unsafe { libc::mkfifo(c.as_ptr(), 0o600) } != 0
        && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST)
    {
        eprintln!("stanley: mkfifo {path}: {}", std::io::Error::last_os_error());
        return None;
    }
    // O_RDWR, not O_RDONLY: a FIFO with no writer polls readable forever
    // with POLLHUP. Holding a writer ourselves keeps poll() blocking.
    let fd = unsafe { libc::open(c.as_ptr(), libc::O_RDWR | libc::O_NONBLOCK | libc::O_CLOEXEC) };
    if fd < 0 {
        eprintln!("stanley: open {path}: {}", std::io::Error::last_os_error());
        return None;
    }
    Some(fd)
}

pub struct Clicks {
    /// Each FIFO and the size of its records.
    fifos: Vec<(RawFd, usize)>,
    children: Vec<Child>,
}

impl Clicks {
    /// Open the FIFOs. None when neither opens; the bar still runs, without
    /// clicks.
    pub fn open() -> Option<Clicks> {
        let fifos: Vec<(RawFd, usize)> =
            paths().into_iter().filter_map(|(path, size)| open(&path).map(|fd| (fd, size))).collect();
        (!fifos.is_empty()).then_some(Clicks { fifos, children: Vec::new() })
    }

    /// One pollfd per FIFO.
    pub fn pollfds(&self) -> Vec<libc::pollfd> {
        self.fifos.iter().map(|&(fd, _)| libc::pollfd { fd, events: libc::POLLIN, revents: 0 }).collect()
    }

    /// Every complete click waiting on the FIFOs. Each write is one record
    /// and below PIPE_BUF, so a read of one record's size gets exactly one;
    /// a short read is dropped: keeping part of a click would misalign every
    /// one after.
    pub fn drain(&mut self) -> Vec<Click> {
        let mut clicks = Vec::new();
        let mut record = [0u8; 6];
        for &(fd, size) in &self.fifos {
            loop {
                let n = unsafe { libc::read(fd, record.as_mut_ptr().cast(), size) };
                if n <= 0 {
                    break;
                }
                if n as usize == size {
                    clicks.push(Click::parse(&record[..size]));
                }
            }
        }
        clicks
    }

    /// Run a click command in its own session, with `BLOCK_BUTTON` set, and
    /// `BLOCK_X` and `BLOCK_WIDTH` when dwl sent the position. Its stdout is
    /// not the bar's: anything it printed would land on dwl's stdin as
    /// status text.
    pub fn run(&mut self, command: &str, click: Click) {
        self.reap();
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg(command).env("BLOCK_BUTTON", click.button.to_string());
        if let Some((x, width)) = click.at {
            cmd.env("BLOCK_X", x.to_string()).env("BLOCK_WIDTH", width.to_string());
        }
        cmd.stdin(Stdio::null()).stdout(Stdio::null());
        unsafe {
            use std::os::unix::process::CommandExt;
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        match cmd.spawn() {
            Ok(child) => self.children.push(child),
            Err(e) => eprintln!("stanley: {command}: {e}"),
        }
    }

    /// Collect finished click commands, so they do not linger as zombies.
    pub fn reap(&mut self) {
        self.children.retain_mut(|c| matches!(c.try_wait(), Ok(None)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records() {
        assert_eq!(Click::parse(&[3, 1]), Click { block: 3, button: 1, at: None });
        assert_eq!(Click::parse(&[3, 1, 0x2c, 0x01, 0x00, 0x0a]), Click { block: 3, button: 1, at: Some((300, 2560)) });
    }
}
