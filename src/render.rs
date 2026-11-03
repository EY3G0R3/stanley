//! Turn readings into the status line: parts, spacing, groups, markers.

use crate::config::{Parts, Section, Settings};
use crate::meter;
use crate::protocol::{is_blank, to_v1, Role, BLOCK, CENTER, INDEX_BASE, LEFT, RIGHT};
use crate::blocks::Reading;

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
    pub section: Section,
    pub text: &'a str,
    pub bare: bool,
}

/// The status line in protocol `version`: all three sections for 2, and for
/// 1 only the right one, since that dwl draws its own clock.
pub fn line(blocks: &[Placed], settings: &Settings, version: u32) -> String {
    if version < 2 {
        return to_v1(&section(blocks, Section::Right, settings));
    }
    let mut out = String::new();
    for (marker, which) in [(LEFT, Section::Left), (CENTER, Section::Center), (RIGHT, Section::Right)] {
        out.push(marker);
        out.push_str(&section(blocks, which, settings));
    }
    out
}

/// One section's blocks. Each carries its marker, then the gap before it, so
/// a click on the gap counts for the widget to its right, as in slstatus.
/// The first has no gap: dwl places a section by its glyphs, and leading
/// spaces would pull the center section off centre.
fn section(blocks: &[Placed], which: Section, settings: &Settings) -> String {
    let mut out = String::new();
    let mut previous: Option<&Placed> = None;
    for block in blocks.iter().filter(|b| b.section == which && !b.text.is_empty()) {
        let gap = match previous {
            None => 0,
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
        let right = |index, group, text, bare| Placed { index, group, section: Section::Right, text, bare };
        let blocks = [
            right(0, 0, "a", false),
            right(1, 0, "", false),
            right(2, 0, "b", false),
            right(3, 1, "c", true),
            right(4, 1, "d", true),
        ];
        assert_eq!(line(&blocks, &s, 1), "\x1f!a\x1f#  b\x1f$    c\x1f% d");
    }

    #[test]
    fn sections_in_order() {
        let s = Settings::default();
        let blocks = [
            Placed { index: 0, group: 0, section: Section::Right, text: "\x02r", bare: false },
            Placed { index: 1, group: 1, section: Section::Center, text: "c", bare: false },
            Placed { index: 2, group: 2, section: Section::Left, text: "l", bare: false },
        ];
        assert_eq!(line(&blocks, &s, 2), "\x1c\x1f#l\x1d\x1f\"c\x1e\x1f!\x02r");
        // Protocol 1: the right section alone, in slstatus's role bytes.
        assert_eq!(line(&blocks, &s, 1), "\x1f!\x04r");
    }
}
