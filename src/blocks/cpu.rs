//! Whole-machine CPU use since the last tick, from /proc/stat.

use super::{icon, popup, Block, Reading};
use crate::meter::Gauge;

pub struct Cpu {
    /// (busy, total) ticks from the last reading.
    last: Option<(u64, u64)>,
}

impl Cpu {
    pub fn new() -> Cpu {
        Cpu { last: None }
    }
}

/// (busy, total) ticks summed over all cores since boot. Busy is user, nice,
/// system, irq and softirq; total adds idle and iowait.
pub(super) fn sample() -> Option<(u64, u64)> {
    let stat = std::fs::read_to_string("/proc/stat").ok()?;
    let mut fields = stat.lines().next()?.split_whitespace().skip(1);
    let mut v = [0u64; 7];
    for slot in &mut v {
        *slot = fields.next()?.parse().ok()?;
    }
    Some((v[0] + v[1] + v[2] + v[5] + v[6], v.iter().sum()))
}

impl Block for Cpu {
    fn click(&mut self, button: u8) -> Option<String> {
        popup(button, "cpu")
    }

    fn read(&mut self) -> Reading {
        let now = sample();
        // The first reading has nothing to measure against: icon, no gauge.
        let gauge = match (self.last, now) {
            (Some((busy0, total0)), Some((busy1, total1))) => {
                let busy = busy1.saturating_sub(busy0);
                let total = total1.saturating_sub(total0);
                (total > 0).then(|| Gauge::new((100 * busy / total) as u32))
            }
            _ => None,
        };
        self.last = now;
        Reading { icon: Some(icon::CPU.into()), name: Some("CPU".into()), gauge, extra: None }
    }
}
