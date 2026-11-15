//! Weekly quota margin at the average rate since the window began.

use super::{icon, popup, Block, Reading};
use crate::protocol::{paint, Role};
use std::{env, fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

pub struct Agents;

fn margin(data: &str, now: f64) -> Option<String> {
    let mut fields = data.split_whitespace();
    let percent: f64 = fields.next()?.parse().ok()?;
    let reset: f64 = fields.next()?.parse().ok()?;
    let week = 7.0 * 86400.0;
    let elapsed = now - (reset - week);
    if fields.next().is_some() || !percent.is_finite() || !reset.is_finite()
        || percent <= 0.0 || percent > 100.0 || reset <= now || elapsed < 86400.0 {
        return None;
    }
    let hours = (elapsed * 100.0 / percent - week) / 3600.0;
    let rounded = hours.round() as i64;
    let text = if rounded == 0 { "0hrs".into() } else { format!("{rounded:+}hrs") };
    Some(paint(Role::Muted, &text))
}

impl Block for Agents {
    fn click(&mut self, button: u8) -> Option<String> {
        popup(button, "agents")
    }

    fn read(&mut self) -> Reading {
        let dir = env::var_os("XDG_STATE_HOME").filter(|s| !s.is_empty()).map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")));
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64();
        let extra = dir.and_then(|dir| {
            let data = fs::read_to_string(dir.join("agent-quota/claude")).ok()?;
            margin(&data, now)
        });
        Reading { icon: Some(icon::AGENTS.into()), name: Some("AI".into()), extra, ..Reading::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_margin_is_muted() {
        for (elapsed, hours) in [(73, -22), (89, 10), (78, -12), (90, 12), (91, 14)] {
            assert_eq!(margin("50 604800", elapsed as f64 * 3600.0),
                Some(paint(Role::Muted, &format!("{hours:+}hrs"))));
        }
        assert_eq!(margin("50 604800", 84.0 * 3600.0), Some(paint(Role::Muted, "0hrs")));
    }

    #[test]
    fn unavailable_forecasts_are_hidden() {
        for data in ["", "0 604800", "NaN 604800", "50 NaN", "101 604800", "50 604800 extra"] {
            assert_eq!(margin(data, 300000.0), None);
        }
        assert_eq!(margin("50 604800", 3600.0), None);
        assert_eq!(margin("50 604800", 604800.0), None);
    }
}
