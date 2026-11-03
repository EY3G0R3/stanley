//! Memory in use, with the tmpfs share marked inside the gauge.

use super::{icon, Reading, Block};
use crate::meter::Gauge;

pub struct Memory;

/// Used and shared memory as percentages of the total. Cached pages are free
/// for the taking, except Shmem: tmpfs lives there and cannot be dropped, so
/// it is added back after Cached is subtracted.
fn sample() -> Option<(u32, u32)> {
    let info = std::fs::read_to_string("/proc/meminfo").ok()?;
    let field = |key: &str| -> Option<u64> {
        info.lines()
            .find_map(|l| l.strip_prefix(key))?
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    };
    let total = field("MemTotal:")?;
    if total == 0 {
        return None;
    }
    let shmem = field("Shmem:")?;
    let used = (total + shmem)
        .saturating_sub(field("MemFree:")?)
        .saturating_sub(field("Buffers:")?)
        .saturating_sub(field("Cached:")?)
        .saturating_sub(field("SReclaimable:")?);
    Some(((100 * used / total) as u32, (100 * shmem / total) as u32))
}

impl Block for Memory {
    fn read(&mut self) -> Reading {
        let gauge = sample().map(|(used, shared)| Gauge { value: used, marked: shared, severity: used });
        Reading { icon: Some(icon::RAM.into()), name: Some("RAM".into()), gauge, extra: None }
    }
}
