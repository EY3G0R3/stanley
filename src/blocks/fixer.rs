//! Whether ~/bin/fix-my-messages runs and watches messages, how many fixes
//! it made today, and how many messages it was too slow for (in red).

use super::{dim, icon, popup, Block, Reading};
use crate::protocol::{paint, Role};
use std::fs;

pub struct Fixer;

/// Whether the wrapper's cgroup still holds the agent and the message stream
/// it reads. The wrapper can outlive them, and then nothing gets checked.
fn watching(pid: i32) -> bool {
    let Some(group) = fs::read_to_string(format!("/proc/{pid}/cgroup"))
        .ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("0::").map(str::to_string)))
    else {
        return false;
    };
    let procs = fs::read_to_string(format!("/sys/fs/cgroup{group}/cgroup.procs")).unwrap_or_default();
    let cmds: Vec<String> = procs
        .lines()
        .filter_map(|p| fs::read(format!("/proc/{p}/cmdline")).ok())
        .map(|c| String::from_utf8_lossy(&c).replace('\0', " "))
        .collect();
    let has = |needle: &str| cmds.iter().any(|c| c.contains(needle));
    has("fix-my-messages.ts") && has("messages list --follow")
}

/// The wrapper keeps its pid, the day and the day's fix and miss counts in
/// $XDG_RUNTIME_DIR/fix-my-messages. A count is 0 when it is from another day.
/// The flag says whether the agent still watches messages.
fn state() -> Option<(u32, u32, bool)> {
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
        return Some((0, 0, watching(pid)));
    }
    let count = |name: &str| {
        fs::read_to_string(format!("{base}/{name}")).ok().and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(0)
    };
    Some((count("count"), count("missed"), watching(pid)))
}

impl Block for Fixer {
    fn click(&mut self, button: u8) -> Option<String> {
        popup(button, "messages")
    }

    fn read(&mut self) -> Reading {
        match state() {
            // Off: a dim icon is the reminder.
            None => Reading { icon: Some(dim(icon::FIXER)), name: Some(dim("MSG")), ..Reading::default() },
            // Running but deaf: the agent or its message stream is gone.
            Some((_, _, false)) => {
                Reading { icon: Some(icon::FIXER.into()), name: Some(paint(Role::Crit, "MSG")), ..Reading::default() }
            }
            Some((fixed, missed, true)) => {
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

#[cfg(test)]
mod tests {
    use super::watching;

    #[test]
    fn a_process_without_the_agent_is_not_watching() {
        assert!(!watching(std::process::id() as i32));
    }

    // The live service: FIXER_PID=$(cat $XDG_RUNTIME_DIR/fix-my-messages/pid)
    #[test]
    #[ignore]
    fn the_live_fixer_is_watching() {
        let pid = std::env::var("FIXER_PID").unwrap().parse().unwrap();
        assert!(watching(pid));
    }
}
