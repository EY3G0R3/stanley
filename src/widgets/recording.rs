//! Whether ~/bin/screen-record is recording: red while it is, dim otherwise.

use super::{dim, icon, Reading, Widget};
use crate::protocol::{paint, Role};
use std::fs;

pub struct Recording;

/// The script keeps its recorder's pid in $XDG_RUNTIME_DIR/screen-record/pid.
/// The comm check is the script's own: a stale file can name a reused pid.
fn recording() -> Option<bool> {
    let dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let pid: u32 = fs::read_to_string(format!("{dir}/screen-record/pid")).ok()?.trim().parse().ok()?;
    let comm = fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    // gpu-screen-recorder's comm is cut to 15 characters.
    Some(matches!(comm.trim(), "gpu-screen-reco" | "wf-recorder"))
}

impl Widget for Recording {
    fn read(&mut self) -> Reading {
        let on = recording().unwrap_or(false);
        let style = |s: &str| if on { paint(Role::Crit, s) } else { dim(s) };
        Reading { icon: Some(style(icon::RECORD)), name: Some(style("REC")), ..Reading::default() }
    }
}
