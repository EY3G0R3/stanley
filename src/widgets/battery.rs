//! The machine's battery, averaged over its batteries, from sysfs.

use super::{icon, Reading, Widget};
use crate::meter::Gauge;
use crate::protocol::{paint, Role};
use std::fs;

pub struct Battery;

/// Below this the battery also spells out its percentage.
const DETAIL_BELOW: u32 = 30;

fn read_trim(path: &str) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

impl Widget for Battery {
    fn read(&mut self) -> Reading {
        let mut sum = 0u32;
        let mut count = 0u32;
        let mut plugged = false;
        if let Ok(dir) = fs::read_dir("/sys/class/power_supply") {
            for entry in dir.flatten() {
                let base = format!("/sys/class/power_supply/{}", entry.file_name().to_string_lossy());
                // Whether power comes in is the adapter's to say: a full
                // battery reports "Full", not "Charging".
                if read_trim(&format!("{base}/type")).as_deref() != Some("Battery") {
                    if read_trim(&format!("{base}/online")).as_deref() == Some("1") {
                        plugged = true;
                    }
                    continue;
                }
                // A wireless mouse is a "Battery" too, with scope "Device".
                if read_trim(&format!("{base}/scope")).as_deref() == Some("Device") {
                    continue;
                }
                if let Some(capacity) = read_trim(&format!("{base}/capacity")).and_then(|s| s.parse::<i64>().ok()) {
                    sum += capacity.clamp(0, 100) as u32;
                    count += 1;
                }
            }
        }

        let capacity = sum.checked_div(count).unwrap_or(0);
        // No battery: still a click target for the power menu.
        if capacity == 0 {
            return Reading { icon: Some(icon::POWER.into()), name: Some("PWR".into()), ..Reading::default() };
        }
        // Full and plugged in is one quiet state, not a full gauge beside a plug.
        if plugged && capacity >= 95 {
            return Reading { icon: Some(icon::PLUG.into()), name: Some("AC".into()), ..Reading::default() };
        }
        Reading {
            icon: Some(if plugged { icon::PLUG } else { icon::BATTERY }.into()),
            name: Some(if plugged { "AC" } else { "BAT" }.into()),
            // A battery is alarming when it is low.
            gauge: Some(Gauge { value: capacity, marked: 0, severity: 100 - capacity }),
            extra: (capacity < DETAIL_BELOW).then(|| paint(Role::Normal, &format!("{capacity}%"))),
        }
    }
}
