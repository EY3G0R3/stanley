//! Clicks from dwl: two bytes, block and button, on a FIFO.
//!
//! dwl knows only where the pointer landed; stanley keeps the mapping from
//! block to command, so a new clickable widget is a line in the bar file and
//! nothing has to be taught to the compositor.

use std::ffi::CString;
use std::os::fd::RawFd;
use std::process::{Child, Command, Stdio};

/// `$SLSTATUS_CLICK_FIFO`, else `$XDG_RUNTIME_DIR/slstatus-click`: the path
/// dwl writes to today. Phase 2 renames it.
fn path() -> Option<String> {
    if let Ok(p) = std::env::var("SLSTATUS_CLICK_FIFO") {
        return Some(p);
    }
    std::env::var("XDG_RUNTIME_DIR").ok().map(|dir| format!("{dir}/slstatus-click"))
}

pub struct Clicks {
    fd: RawFd,
    children: Vec<Child>,
}

impl Clicks {
    /// Open the FIFO, creating it if needed. None when there is nowhere to
    /// put it; the bar still runs, without clicks.
    pub fn open() -> Option<Clicks> {
        let path = path()?;
        let c = CString::new(path.clone()).ok()?;
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
        Some(Clicks { fd, children: Vec::new() })
    }

    pub fn fd(&self) -> RawFd {
        self.fd
    }

    /// Every complete click waiting on the FIFO, as (block, button). A short
    /// read is dropped: keeping half a click would misalign every one after.
    pub fn drain(&mut self) -> Vec<(u8, u8)> {
        let mut clicks = Vec::new();
        let mut pair = [0u8; 2];
        loop {
            let n = unsafe { libc::read(self.fd, pair.as_mut_ptr().cast(), 2) };
            if n <= 0 {
                break;
            }
            if n == 2 {
                clicks.push((pair[0], pair[1]));
            }
        }
        clicks
    }

    /// Run a click command in its own session, with `BLOCK_BUTTON` set.
    /// Its stdout is not the bar's: anything it printed would land on dwl's
    /// stdin as status text.
    pub fn run(&mut self, command: &str, button: u8) {
        self.reap();
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg(command).env("BLOCK_BUTTON", button.to_string());
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
