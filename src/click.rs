//! Clicks from dwl, on a FIFO.
//!
//! dwl knows only where the pointer landed; stanley keeps the mapping from
//! block to command, so a new clickable widget is a line in the bar file and
//! nothing has to be taught to the compositor.
//!
//! The FIFO is `$XDG_RUNTIME_DIR/stanley-click`. A click is six bytes: block,
//! button, then x and the monitor's width as little-endian u16.

use std::ffi::CString;
use std::os::fd::RawFd;
use std::process::{Child, Command, Stdio};

/// One click. `x` and `width` are the pointer's x and the monitor's width,
/// in monitor-local logical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Click {
    pub block: u8,
    pub button: u8,
    pub x: u16,
    pub width: u16,
}

impl Click {
    fn parse(r: &[u8; 6]) -> Click {
        Click {
            block: r[0],
            button: r[1],
            x: u16::from_le_bytes([r[2], r[3]]),
            width: u16::from_le_bytes([r[4], r[5]]),
        }
    }
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
    fd: RawFd,
    children: Vec<Child>,
}

impl Clicks {
    /// Open the FIFO. None when it cannot be opened; the bar still runs,
    /// without clicks.
    pub fn open() -> Option<Clicks> {
        let dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
        open(&format!("{dir}/stanley-click")).map(|fd| Clicks { fd, children: Vec::new() })
    }

    pub fn fd(&self) -> RawFd {
        self.fd
    }

    /// Every complete click waiting on the FIFO. Each write is one click and
    /// below PIPE_BUF, so a six-byte read gets exactly one; a short read is
    /// dropped: keeping part of a click would misalign every one after.
    pub fn drain(&mut self) -> Vec<Click> {
        let mut clicks = Vec::new();
        let mut record = [0u8; 6];
        loop {
            let n = unsafe { libc::read(self.fd, record.as_mut_ptr().cast(), record.len()) };
            if n <= 0 {
                break;
            }
            if n as usize == record.len() {
                clicks.push(Click::parse(&record));
            }
        }
        clicks
    }

    /// Run a click command in its own session, with `BLOCK_BUTTON`, `BLOCK_X`
    /// and `BLOCK_WIDTH` set. Its stdout is not the bar's: anything it printed
    /// would land on dwl's stdin as status text.
    pub fn run(&mut self, command: &str, click: Click) {
        self.reap();
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c")
            .arg(command)
            .env("BLOCK_BUTTON", click.button.to_string())
            .env("BLOCK_X", click.x.to_string())
            .env("BLOCK_WIDTH", click.width.to_string());
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
        assert_eq!(Click::parse(&[3, 1, 0x2c, 0x01, 0x00, 0x0a]), Click { block: 3, button: 1, x: 300, width: 2560 });
    }
}
