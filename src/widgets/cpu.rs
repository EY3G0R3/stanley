//! Whole-machine CPU use since the last tick, from /proc/stat.

use super::{icon, Reading, Widget};
use crate::meter::Gauge;

pub struct Cpu {
    /// user nice system idle iowait irq softirq, from the last reading.
    last: Option<[u64; 7]>,
}

impl Cpu {
    pub fn new() -> Cpu {
        Cpu { last: None }
    }
}

fn sample() -> Option<[u64; 7]> {
    let stat = std::fs::read_to_string("/proc/stat").ok()?;
    let mut fields = stat.lines().next()?.split_whitespace().skip(1);
    let mut out = [0u64; 7];
    for slot in &mut out {
        *slot = fields.next()?.parse().ok()?;
    }
    Some(out)
}

impl Widget for Cpu {
    fn read(&mut self) -> Reading {
        let now = sample();
        // The first reading has nothing to measure against: icon, no gauge.
        let gauge = match (self.last, now) {
            (Some(a), Some(b)) => {
                let delta = |i: usize| b[i].saturating_sub(a[i]);
                let total: u64 = (0..7).map(delta).sum();
                let busy = delta(0) + delta(1) + delta(2) + delta(5) + delta(6);
                (total > 0).then(|| Gauge::new((100 * busy / total) as u32))
            }
            _ => None,
        };
        self.last = now;
        Reading { icon: Some(icon::CPU.into()), name: Some("CPU".into()), gauge, extra: None }
    }
}
