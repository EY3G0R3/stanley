//! Whether ~/bin/fix-my-messages runs, and how many fixes it made today.

use super::{dim, icon, Reading, Widget};
use std::fs;

pub struct Fixer;

/// The wrapper keeps its pid, the day and the day's count in
/// $XDG_RUNTIME_DIR/fix-my-messages.
fn state() -> Option<Option<u32>> {
    let dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let base = format!("{dir}/fix-my-messages");
    let pid: i32 = fs::read_to_string(format!("{base}/pid")).ok()?.trim().parse().ok()?;
    if pid <= 0 || unsafe { libc::kill(pid, 0) } != 0 {
        return None;
    }
    let today = {
        let now = unsafe { libc::time(std::ptr::null_mut()) };
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        unsafe { libc::localtime_r(&now, &mut tm) };
        format!("{:04}-{:02}-{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday)
    };
    let day = fs::read_to_string(format!("{base}/day")).ok().map(|s| s.trim().to_string());
    let count = fs::read_to_string(format!("{base}/count")).ok().and_then(|s| s.trim().parse::<u32>().ok());
    Some(if day.as_deref() == Some(today.as_str()) { count.filter(|&c| c > 0) } else { None })
}

impl Widget for Fixer {
    fn read(&mut self) -> Reading {
        match state() {
            // Off: a dim icon is the reminder.
            None => Reading { icon: Some(dim(icon::FIXER)), name: Some(dim("MSG")), ..Reading::default() },
            Some(count) => Reading {
                icon: Some(icon::FIXER.into()),
                name: Some("MSG".into()),
                gauge: None,
                extra: count.map(|c| dim(&c.to_string())),
            },
        }
    }
}
