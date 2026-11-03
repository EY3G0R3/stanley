//! This user's busiest programs, after the top-processes widget on the
//! Omarchy bar: `name(N%)` joined by commas, re-ranked every few seconds.
//!
//! N is a share of the whole machine, as in the CPU popup. The colors go by
//! cores instead, as in top: a program past half a core turns yellow whatever
//! the core count. Time is measured over the whole window since the last
//! reading, which steadies a ranking redrawn only every few seconds.
//!
//! Processes of one name count as one program, shown `name×K(N%)`: eleven
//! fancylog processes at 1% each are one 11% fancylog, not eleven ties.

use super::{dim, Reading, Widget};
use crate::protocol::{clean, paint, Role};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::time::{Duration, Instant};

const COUNT: usize = 3;
const EVERY: Duration = Duration::from_secs(5);
/// Shares of one core.
const WARN_AT: u32 = 50;
const CRIT_AT: u32 = 80;

pub struct Top {
    /// pid -> (start time, CPU ticks) at the last reading.
    last: HashMap<u32, (u64, u64)>,
    taken: Option<Instant>,
    hz: f64,
    cores: f64,
    uid: u32,
}

impl Top {
    pub fn new() -> Top {
        Top {
            last: HashMap::new(),
            taken: None,
            hz: (unsafe { libc::sysconf(libc::_SC_CLK_TCK) }).max(1) as f64,
            cores: (unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) }).max(1) as f64,
            uid: unsafe { libc::geteuid() },
        }
    }
}

/// One process: (command, start time, utime + stime). The command sits in
/// parentheses and may contain spaces and parentheses itself, so the fields
/// are counted from the last ')'.
fn process(pid: &str) -> Option<(String, u64, u64)> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let comm = stat.get(open + 1..close)?.to_string();
    // After ')': state ppid pgrp session tty_nr tpgid flags minflt cminflt
    // majflt cmajflt utime stime cutime cstime priority nice num_threads
    // itrealvalue starttime
    let fields: Vec<&str> = stat[close + 1..].split_whitespace().collect();
    let utime: u64 = fields.get(11)?.parse().ok()?;
    let stime: u64 = fields.get(12)?.parse().ok()?;
    let start: u64 = fields.get(19)?.parse().ok()?;
    Some((comm, start, utime + stime))
}

impl Widget for Top {
    fn interval(&self) -> Option<Duration> {
        Some(EVERY)
    }

    fn read(&mut self) -> Reading {
        let now = Instant::now();
        let mut current = HashMap::new();
        // name -> (ticks in the window, processes)
        let mut programs: HashMap<String, (u64, u32)> = HashMap::new();
        if let Ok(dir) = fs::read_dir("/proc") {
            for entry in dir.flatten() {
                let name = entry.file_name();
                let Some(pid_str) = name.to_str().filter(|s| s.starts_with(|c: char| c.is_ascii_digit())) else {
                    continue;
                };
                // Only this user's programs: a click on the bar cannot act on
                // kernel threads or system daemons.
                if entry.metadata().map(|m| m.uid()).ok() != Some(self.uid) {
                    continue;
                }
                let (Ok(pid), Some((comm, start, ticks))) = (pid_str.parse::<u32>(), process(pid_str)) else {
                    continue;
                };
                // A process absent last time, or whose start time changed (its
                // pid was reused), spent all of its time inside the window.
                let spent = match self.last.get(&pid) {
                    Some(&(s, t)) if s == start && t <= ticks => ticks - t,
                    _ => ticks,
                };
                current.insert(pid, (start, ticks));
                let program = programs.entry(comm).or_insert((0, 0));
                program.0 += spent;
                program.1 += 1;
            }
        }
        let elapsed = self.taken.map(|t| now.duration_since(t).as_secs_f64());
        self.last = current;
        self.taken = Some(now);

        // The first reading has nothing to measure against.
        let Some(elapsed) = elapsed.filter(|&e| e > 0.0) else {
            return Reading { extra: Some(dim("sampling…")), ..Reading::default() };
        };

        let mut ranked: Vec<(String, u64, u32)> = programs.into_iter().map(|(n, (t, c))| (n, t, c)).collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let mut out = String::new();
        for (i, (comm, ticks, count)) in ranked.iter().take(COUNT).enumerate() {
            let of_core = (*ticks as f64 * 100.0 / (self.hz * elapsed) + 0.5) as u32;
            let share = *ticks as f64 * 100.0 / (self.hz * elapsed * self.cores);
            let role = if of_core > CRIT_AT {
                Role::Crit
            } else if of_core > WARN_AT {
                Role::Warn
            } else {
                Role::Muted
            };
            if i > 0 {
                out.push_str(&dim(", "));
            }
            let comm = clean(comm);
            let label = if *count > 1 { format!("{comm}×{count}({share:.1}%)") } else { format!("{comm}({share:.1}%)") };
            out.push_str(&paint(role, &label));
        }
        Reading { extra: (!out.is_empty()).then_some(out), ..Reading::default() }
    }
}
