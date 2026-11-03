//! The machine's name, muted: it says which machine, it is not there to read.

use super::{icon, Reading, Widget};
use crate::protocol::{clean, paint, Role};
use std::time::Duration;

pub struct Hostname;

fn hostname() -> Option<String> {
    let mut buf = [0u8; 256];
    if unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) } != 0 {
        return None;
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Some(clean(&String::from_utf8_lossy(&buf[..end])))
}

impl Widget for Hostname {
    fn read(&mut self) -> Reading {
        Reading {
            icon: Some(paint(Role::Muted, icon::HOST)),
            name: hostname().map(|h| paint(Role::Muted, &h)),
            ..Reading::default()
        }
    }

    fn interval(&self) -> Option<Duration> {
        Some(Duration::from_secs(60))
    }
}
