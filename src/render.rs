//! Turn readings into the status line: parts, spacing, groups, markers.

use crate::config::{Parts, Settings};
use crate::meter;
use crate::protocol::{is_blank, Role, BLOCK, INDEX_BASE};
use crate::widgets::Reading;

/// One widget's text: its listed parts that have something to show, one space
/// apart. Empty when none do, and then the widget drops out of the bar.
/// `bare` is true when only the icon is drawn.
pub fn block(parts: Parts, reading: &Reading, settings: &Settings) -> (String, bool) {
    let mut pieces: Vec<String> = Vec::new();
    let mut only_icon = true;
    let mut push = |text: String, is_icon: bool, pieces: &mut Vec<String>| {
        if !is_blank(&text) {
            only_icon &= is_icon;
            pieces.push(text);
        }
    };
    if parts.icon {
        if let Some(icon) = &reading.icon {
            push(icon.clone(), true, &mut pieces);
        }
    }
    if parts.name {
        if let Some(name) = &reading.name {
            push(name.clone(), false, &mut pieces);
        }
    }
    if parts.gauge {
        if let Some(gauge) = reading.gauge {
            push(meter::render(&settings.meter, gauge), false, &mut pieces);
        }
    }
    if parts.extra {
        if let Some(extra) = &reading.extra {
            push(extra.clone(), false, &mut pieces);
        }
    }
    if pieces.is_empty() {
        return (String::new(), false);
    }
    let mut text = pieces.join(" ");
    text.push(Role::Normal.ch());
    (text, only_icon)
}

/// One rendered widget, in bar order.
pub struct Placed<'a> {
    /// Position in the bar file, 0-based; clicks come back with it.
    pub index: usize,
    pub group: usize,
    pub text: &'a str,
    pub bare: bool,
}

/// The status line. Each block carries its marker, then the gap before it,
/// so a click on the gap counts for the widget to its right, as in slstatus.
pub fn line(blocks: &[Placed], settings: &Settings) -> String {
    let mut out = String::new();
    let mut previous: Option<&Placed> = None;
    for block in blocks.iter().filter(|b| !b.text.is_empty()) {
        let gap = match previous {
            Some(p) if p.group == block.group => {
                if p.bare && block.bare {
                    settings.icon_gap
                } else {
                    settings.gap
                }
            }
            _ => settings.group_gap,
        };
        out.push(BLOCK);
        out.push(char::from_u32(INDEX_BASE + block.index as u32 + 1).unwrap_or(' '));
        out.extend(std::iter::repeat_n(' ', gap));
        out.push_str(block.text);
        previous = Some(block);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meter::Gauge;

    const ALL: Parts = Parts { icon: true, name: true, gauge: true, extra: true };

    fn visible(s: &str) -> String {
        s.chars().filter(|&c| c >= ' ').collect()
    }

    #[test]
    fn parts_select_what_shows() {
        let r = Reading { icon: Some("I".into()), name: Some("CPU".into()), gauge: Some(Gauge::new(0)), extra: None };
        let s = Settings::default();
        let icon_only = Parts { icon: true, name: false, gauge: false, extra: false };
        assert_eq!(block(icon_only, &r, &s), ("I\x01".to_string(), true));
        let text = Parts { icon: false, name: true, gauge: false, extra: false };
        assert_eq!(visible(&block(text, &r, &s).0), "CPU");
        assert_eq!(visible(&block(ALL, &r, &s).0), "I CPU ──────");
    }

    #[test]
    fn empty_reading_drops_out() {
        assert_eq!(block(ALL, &Reading::default(), &Settings::default()).0, "");
    }

    #[test]
    fn gaps_and_markers() {
        let s = Settings::default();
        let blocks = [
            Placed { index: 0, group: 0, text: "a", bare: false },
            Placed { index: 1, group: 0, text: "", bare: false },
            Placed { index: 2, group: 0, text: "b", bare: false },
            Placed { index: 3, group: 1, text: "c", bare: true },
            Placed { index: 4, group: 1, text: "d", bare: true },
        ];
        assert_eq!(line(&blocks, &s), "\x1f!    a\x1f#  b\x1f$    c\x1f% d");
    }
}
