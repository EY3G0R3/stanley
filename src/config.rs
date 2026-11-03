//! The bar file: which widgets, in what order, showing which parts.
//!
//! ```text
//! set meter-style box          # settings first
//!
//! cpu     icon gauge
//! disk:/  icon gauge     click: stanley-popup disk
//!
//! memory  icon           # a blank line above starts a new group
//! ```
//!
//! A line whose first non-blank character is `#` is a comment. There are no
//! comments at the end of a line: a click command may contain `#`.

use crate::meter::{Meter, Style};
use std::path::PathBuf;

/// Which parts of a widget's reading appear on the bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Parts {
    pub icon: bool,
    pub name: bool,
    pub gauge: bool,
    pub extra: bool,
}

impl Parts {
    const ALL: Parts = Parts { icon: true, name: true, gauge: true, extra: true };
    const NONE: Parts = Parts { icon: false, name: false, gauge: false, extra: false };
}

/// Where on the bar a widget sits. dwl lays the sections out; see
/// docs/design.md.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Left,
    Center,
    Right,
}

/// One widget line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub widget: String,
    pub arg: Option<String>,
    pub parts: Parts,
    pub click: Option<String>,
    /// Lines separated by a blank line are in different groups.
    pub group: usize,
    pub section: Section,
}

/// `set` lines.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub interval_ms: u64,
    pub meter: Meter,
    /// Spaces between widgets in a group.
    pub gap: usize,
    /// Spaces before each group.
    pub group_gap: usize,
    /// Spaces between two widgets that both show only an icon.
    pub icon_gap: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { interval_ms: 1000, meter: Meter::default(), gap: 2, group_gap: 4, icon_gap: 1 }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Config {
    pub settings: Settings,
    pub entries: Vec<Entry>,
}

/// The built-in bar, used when there is no bar file.
pub const DEFAULT: &str = include_str!("../bar.example");

/// `$XDG_CONFIG_HOME/stanley/bar`, else `~/.config/stanley/bar`.
pub fn default_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir).join("stanley/bar"));
    }
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config/stanley/bar"))
}

/// Parse a bar file. Bad lines are reported and skipped, so one typo does not
/// take the whole bar down.
pub fn parse(text: &str, warn: &mut dyn FnMut(usize, String)) -> Config {
    let mut config = Config::default();
    let mut group = 0;
    let mut group_open = false;
    // Phase 1 bar files have no section lines, and all of it is the right.
    let mut section = Section::Right;

    for (n, raw) in text.lines().enumerate() {
        let lineno = n + 1;
        let line = raw.trim();
        if line.is_empty() {
            if group_open {
                group += 1;
                group_open = false;
            }
            continue;
        }
        if line.starts_with('#') {
            continue;
        }

        let (spec, click) = match line.find("click:") {
            Some(i) if i == 0 || line[..i].ends_with(char::is_whitespace) => {
                let command = line[i + "click:".len()..].trim();
                (line[..i].trim(), (!command.is_empty()).then(|| command.to_string()))
            }
            _ => (line, None),
        };
        let mut words = spec.split_whitespace();
        let Some(first) = words.next() else {
            warn(lineno, "click: without a widget".into());
            continue;
        };

        if first == "set" {
            let key = words.next().unwrap_or("");
            let value = words.next().unwrap_or("");
            if let Err(e) = set(&mut config.settings, key, value) {
                warn(lineno, e);
            }
            continue;
        }

        if first == "section" {
            match words.next().unwrap_or("") {
                "left" => section = Section::Left,
                "center" => section = Section::Center,
                "right" => section = Section::Right,
                other => warn(lineno, format!("unknown section {other:?}: use left, center or right")),
            }
            continue;
        }

        let (widget, arg) = match first.split_once(':') {
            Some((w, a)) => (w, (!a.is_empty()).then(|| a.to_string())),
            None => (first, None),
        };
        let mut parts = Parts::NONE;
        let mut listed = false;
        let mut bad = false;
        for word in words {
            match word {
                "icon" => parts.icon = true,
                "name" => parts.name = true,
                "gauge" => parts.gauge = true,
                "extra" => parts.extra = true,
                _ => {
                    warn(lineno, format!("unknown part {word:?}: use icon, name, gauge or extra"));
                    bad = true;
                }
            }
            listed = true;
        }
        if bad {
            continue;
        }
        config.entries.push(Entry {
            widget: widget.to_string(),
            arg,
            parts: if listed { parts } else { Parts::ALL },
            click,
            group,
            section,
        });
        group_open = true;
    }
    config
}

fn set(settings: &mut Settings, key: &str, value: &str) -> Result<(), String> {
    let number = || value.parse::<u64>().map_err(|_| format!("set {key}: {value:?} is not a number"));
    match key {
        "interval" => settings.interval_ms = number()?.max(100),
        "meter-style" => {
            settings.meter.style =
                Style::parse(value).ok_or_else(|| format!("set meter-style: unknown style {value:?}"))?
        }
        "meter-cells" => settings.meter.cells = number()?.min(40) as u32,
        "warn-at" => settings.meter.warn_at = number()?.min(100) as u32,
        "crit-at" => settings.meter.crit_at = number()?.min(100) as u32,
        "gap" => settings.gap = number()?.min(20) as usize,
        "group-gap" => settings.group_gap = number()?.min(20) as usize,
        "icon-gap" => settings.icon_gap = number()?.min(20) as usize,
        _ => return Err(format!("unknown setting {key:?}")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(text: &str) -> Config {
        parse(text, &mut |n, e| panic!("line {n}: {e}"))
    }

    #[test]
    fn widgets_parts_groups_and_clicks() {
        let c = parse_ok(
            "# comment\nset gap 3\n\ncpu icon gauge  click: stanley-popup cpu # not a comment\n\
             disk:/home\n\n\nmemory icon\n",
        );
        assert_eq!(c.settings.gap, 3);
        assert_eq!(c.entries.len(), 3);
        let cpu = &c.entries[0];
        assert_eq!(cpu.widget, "cpu");
        assert_eq!(cpu.parts, Parts { icon: true, name: false, gauge: true, extra: false });
        assert_eq!(cpu.click.as_deref(), Some("stanley-popup cpu # not a comment"));
        let disk = &c.entries[1];
        assert_eq!(disk.arg.as_deref(), Some("/home"));
        assert_eq!(disk.parts, Parts::ALL);
        assert_eq!(disk.group, cpu.group);
        // Two blank lines still make one group break.
        assert_eq!(c.entries[2].group, cpu.group + 1);
    }

    #[test]
    fn sections() {
        let c = parse_ok("cpu\nsection left\nhostname\nsection center\nclock\nsection right\nbattery\n");
        let sections: Vec<Section> = c.entries.iter().map(|e| e.section).collect();
        assert_eq!(sections, [Section::Right, Section::Left, Section::Center, Section::Right]);
    }

    #[test]
    fn bad_lines_are_skipped() {
        let mut errors = Vec::new();
        let c = parse("cpu icon gauje\nset nope 1\nset meter-style wobbly\ndisk\n", &mut |n, e| {
            errors.push((n, e))
        });
        assert_eq!(c.entries.len(), 1);
        assert_eq!(errors.iter().map(|e| e.0).collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn default_bar_parses() {
        let c = parse_ok(DEFAULT);
        assert!(!c.entries.is_empty());
    }
}
