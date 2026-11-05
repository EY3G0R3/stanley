//! This user's busiest programs, after the top-processes widget on the
//! Omarchy bar: `name(N%)` joined by commas, re-ranked every few seconds.
//!
//! N is a share of the whole machine, as in the CPU gauge. The colors go by
//! cores instead, as in top: a program past half a core turns yellow whatever
//! the core count. Time is measured over the whole window since the last
//! reading, which steadies a ranking redrawn only every few seconds.
//!
//! Processes of one name count as one program, shown `name×K(N%)`: eleven
//! fancylog processes at 1% each are one 11% fancylog, not eleven ties.
//!
//! Processes that started and exited inside the window are gone from /proc
//! before they can be read. The machine's busy time no live process accounts
//! for is theirs, so it ranks as one more entry, `exited(N%)`: a test suite
//! forking thousands of short processes would otherwise leave the gauge at
//! 99% and this list near 25%.

use super::{cpu, dim, popup, Block, Reading};
use crate::protocol::{clean, paint, Role};
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::time::Duration;

const COUNT: usize = 3;
const EVERY: Duration = Duration::from_secs(5);
/// Shares of one core.
const WARN_AT: u32 = 50;
const CRIT_AT: u32 = 80;

pub struct Top {
    /// pid -> (start time, CPU ticks) at the last reading.
    last: HashMap<u32, (u64, u64)>,
    /// Machine (busy, total) ticks at the last reading.
    machine: Option<(u64, u64)>,
    cores: f64,
    uid: u32,
}

impl Top {
    pub fn new() -> Top {
        Top {
            last: HashMap::new(),
            machine: None,
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

impl Block for Top {
    fn click(&mut self, button: u8) -> Option<String> {
        match button {
            // dwl parks btop on a scratchpad tag that only its own
            // togglescratch shows, so the click types that binding through
            // the virtual keyboard, staying in step with Super+Escape. It has
            // to change if that binding does.
            1 => Some("wtype -M logo -k Escape -m logo".into()),
            3 => popup(1, "cpu"),
            _ => None,
        }
    }

    fn interval(&self) -> Option<Duration> {
        Some(EVERY)
    }

    fn read(&mut self) -> Reading {
        let machine = cpu::sample();
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
        let window = match (self.machine, machine) {
            (Some((b0, t0)), Some((b1, t1))) => Some((b1.saturating_sub(b0), t1.saturating_sub(t0))),
            _ => None,
        };
        self.last = current;
        self.machine = machine;

        // The first reading has nothing to measure against.
        let Some((busy, total)) = window.filter(|&(_, t)| t > 0) else {
            return Reading { extra: Some(dim("sampling…")), ..Reading::default() };
        };

        let ranked = rank(programs, busy);
        let mut out = String::new();
        for (i, (comm, ticks, count)) in ranked.iter().take(COUNT).enumerate() {
            // Ticks and total are both summed over all cores.
            let share = *ticks as f64 * 100.0 / total as f64;
            let of_core = (share * self.cores + 0.5) as u32;
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

/// Programs as (name, ticks, processes), busiest first, with the machine's
/// busy ticks no live process accounts for added as `exited`.
fn rank(programs: HashMap<String, (u64, u32)>, busy: u64) -> Vec<(String, u64, u32)> {
    let seen: u64 = programs.values().map(|&(t, _)| t).sum();
    let mut ranked: Vec<(String, u64, u32)> = programs.into_iter().map(|(n, (t, c))| (n, t, c)).collect();
    let exited = busy.saturating_sub(seen);
    if exited > 0 {
        ranked.push(("exited".into(), exited, 1));
    }
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unaccounted_busy_time_ranks_as_exited() {
        let programs = HashMap::from([("kitty".to_string(), (30, 1)), ("bun".to_string(), (10, 4))]);
        let ranked = rank(programs, 100);
        assert_eq!(ranked[0], ("exited".to_string(), 60, 1));
        assert_eq!(ranked[1].0, "kitty");
        assert_eq!(ranked[2].0, "bun");
    }

    #[test]
    fn no_exited_entry_when_live_processes_cover_busy_time() {
        let programs = HashMap::from([("kitty".to_string(), (30, 1))]);
        assert_eq!(rank(programs, 25), vec![("kitty".to_string(), 30, 1)]);
    }
}
