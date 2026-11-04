//! Whether ~/bin/fix-my-messages runs, how many fixes it made today, and
//! how many messages it was too slow for (in red).

use super::{dim, icon, popup, Block, Reading};
use crate::protocol::{paint, Role};
use std::fs;

pub struct Fixer;

/// The wrapper keeps its pid, the day and the day's fix and miss counts in
/// $XDG_RUNTIME_DIR/fix-my-messages. A count is 0 when it is from another day.
fn state() -> Option<(u32, u32)> {
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
    if day.as_deref() != Some(today.as_str()) {
        return Some((0, 0));
    }
    let count = |name: &str| {
        fs::read_to_string(format!("{base}/{name}")).ok().and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(0)
    };
    Some((count("count"), count("missed")))
}

impl Block for Fixer {
    fn click(&mut self, button: u8) -> Option<String> {
        popup(button, "messages")
    }

    fn read(&mut self) -> Reading {
        match state() {
            // Off: a dim icon is the reminder.
            None => Reading { icon: Some(dim(icon::FIXER)), name: Some(dim("MSG")), ..Reading::default() },
            Some((fixed, missed)) => {
                let mut extra = String::new();
                if fixed > 0 {
                    extra += &dim(&fixed.to_string());
                }
                if missed > 0 {
                    if fixed > 0 {
                        extra += " ";
                    }
                    extra += &paint(Role::Crit, &format!("!{missed}"));
                }
                Reading {
                    icon: Some(icon::FIXER.into()),
                    name: Some("MSG".into()),
                    gauge: None,
                    extra: (!extra.is_empty()).then_some(extra),
                }
            }
        }
    }
}
