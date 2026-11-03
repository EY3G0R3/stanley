//! Gauges drawn as text, ported from slstatus's meter.c.
//!
//! One style renders every gauge on the bar: a meter is read by comparison
//! with the meters beside it, and two visual languages make that work rather
//! than automatic.

use crate::protocol::Role;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    /// Two-state tiles: ▰▰▰▱▱
    Tiles,
    /// Solid bar painted as background, eighth-block edge.
    Blocks,
    /// Box-drawing line, thin and vertically centred: ━━╾──
    Box,
    /// Braille dots on the baseline: ⣤⣤⡄⣀⣀
    Dots,
    /// Box line with a flame at the fill's tip.
    Flame,
    /// Box line with an exhaust plume at the fill's tip.
    Jet,
    /// One cell whose height is the reading: ▁▂▃▄▅▆▇█
    Spark,
    /// One braille cell: the reading and its marked share side by side.
    Twin,
    /// Solid bar with a flame at its tip.
    BlockFlame,
    /// Solid bar with an exhaust plume at its tip.
    BlockJet,
}

impl Style {
    pub fn parse(name: &str) -> Option<Style> {
        Some(match name {
            "tiles" => Style::Tiles,
            "blocks" => Style::Blocks,
            "box" => Style::Box,
            "dots" => Style::Dots,
            "flame" => Style::Flame,
            "jet" => Style::Jet,
            "spark" => Style::Spark,
            "twin" => Style::Twin,
            "block-flame" => Style::BlockFlame,
            "block-jet" => Style::BlockJet,
            _ => return None,
        })
    }
}

/// Bar-wide gauge settings, from `set` lines in the bar file.
#[derive(Clone, Copy, Debug)]
pub struct Meter {
    pub style: Style,
    pub cells: u32,
    pub warn_at: u32,
    pub crit_at: u32,
}

impl Default for Meter {
    fn default() -> Self {
        Meter { style: Style::Box, cells: 6, warn_at: 60, crit_at: 85 }
    }
}

/// One gauge's reading.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gauge {
    /// 0..=100.
    pub value: u32,
    /// The trailing share of `value` drawn in the accent role, or 0.
    pub marked: u32,
    /// What picks the color. Usually `value`; a battery is alarming when it
    /// is low, so it passes 100 - value.
    pub severity: u32,
}

impl Gauge {
    pub fn new(value: u32) -> Gauge {
        Gauge { value, marked: 0, severity: value }
    }
}

// Powerline Extra tips, in the private use area.
const PL_FLAME: &str = "\u{e0c0}";
const PL_JET: &str = "\u{e0c8}";
const PL_NOSE: &str = "\u{e0b6}"; // a left-bulging semicircle

const EIGHTHS: &[&str] = &["", "▏", "▎", "▍", "▌", "▋", "▊", "▉"];
const HEIGHTS: &[&str] = &["", "▁", "▂", "▃", "▄", "▅", "▆", "▇"];
const BOX_HALF: &[&str] = &["", "╾"];
const DOT_HALF: &[&str] = &["", "⡄"];

/// A style is a set of cells plus the role each run is drawn in.
///
/// `steps` is how many states one cell shows. `partial` holds the
/// intermediate cells, indexed 1..steps-1. `tip` marks where the fill ends and
/// costs the cell it sits in. `tonal` styles color the fill by severity;
/// `painted` says that fill is a background. `plain` drops the marked share.
struct Spec {
    steps: u32,
    tonal: bool,
    painted: bool,
    plain: bool,
    fixed_cells: u32,
    custom: Option<fn(u32, u32, &mut String)>,
    head: Option<&'static str>,
    fill: &'static str,
    accent: &'static str,
    trough: &'static str,
    tip: Option<&'static str>,
    partial: Option<&'static [&'static str]>,
    fill_role: Role,
    edge_role: Role,
    accent_role: Role,
    trough_role: Role,
}

const fn line(tip: Option<&'static str>, steps: u32, partial: Option<&'static [&'static str]>) -> Spec {
    Spec {
        steps,
        tonal: true,
        painted: false,
        plain: false,
        fixed_cells: 0,
        custom: None,
        head: None,
        fill: "━",
        accent: "━",
        trough: "─",
        tip,
        partial,
        fill_role: Role::Normal,
        edge_role: Role::Normal,
        accent_role: Role::Mark,
        trough_role: Role::Dim,
    }
}

const fn painted(tip: &'static str) -> Spec {
    Spec {
        steps: 1,
        tonal: true,
        painted: true,
        plain: true,
        fixed_cells: 0,
        custom: None,
        head: Some(PL_NOSE),
        fill: " ",
        accent: "█",
        trough: " ",
        tip: Some(tip),
        partial: None,
        fill_role: Role::FillBg,
        edge_role: Role::FillEdge,
        accent_role: Role::Mark,
        trough_role: Role::Normal,
    }
}

fn spec(style: Style) -> Spec {
    match style {
        Style::Tiles => Spec {
            fill: "▰",
            accent: "▰",
            trough: "▱",
            trough_role: Role::Normal,
            ..line(None, 1, None)
        },
        Style::Blocks => Spec {
            tonal: false,
            fill: " ",
            accent: "█",
            trough: " ",
            fill_role: Role::FillBg,
            edge_role: Role::FillEdge,
            accent_role: Role::Accent,
            trough_role: Role::FillEdge,
            ..line(None, 8, Some(EIGHTHS))
        },
        Style::Box => line(None, 2, Some(BOX_HALF)),
        Style::Dots => Spec {
            fill: "⣤",
            accent: "⣤",
            trough: "⣀",
            ..line(None, 2, Some(DOT_HALF))
        },
        Style::Flame => line(Some(PL_FLAME), 1, None),
        Style::Jet => line(Some(PL_JET), 1, None),
        // The reading is the cell's height, so the gauge is one column wide.
        Style::Spark => Spec {
            plain: true,
            fixed_cells: 1,
            fill: "█",
            accent: "█",
            trough: "▁",
            ..line(None, 8, Some(HEIGHTS))
        },
        Style::Twin => Spec {
            fixed_cells: 1,
            custom: Some(twin),
            ..line(None, 4, None)
        },
        Style::BlockFlame => painted(PL_FLAME),
        Style::BlockJet => painted(PL_JET),
    }
}

/// A braille cell is two columns of four dots: the reading in one, its
/// marked share in the other. The dot bits run down the left column, then the
/// right, so a level is a prefix of them.
fn twin(value: u32, marked: u32, out: &mut String) {
    const LEFT: [u32; 5] = [0x00, 0x40, 0x44, 0x46, 0x47];
    const RIGHT: [u32; 5] = [0x00, 0x80, 0xa0, 0xb0, 0xb8];
    let lv = ((value * 4 + 50) / 100).min(4) as usize;
    let mut mv = if marked > 0 { (marked * 4 + 50) / 100 } else { 0 };
    if marked > 0 && mv == 0 {
        mv = 1;
    }
    let mv = mv.min(4) as usize;
    let mut bits = if marked > 0 { LEFT[lv] | RIGHT[mv] } else { LEFT[lv] | RIGHT[lv] };
    // An empty gauge keeps its baseline dots, so quiet reads as zero.
    if bits == 0 {
        bits = 0xc0;
    }
    out.push(char::from_u32(0x2800 + bits).unwrap_or('?'));
}

/// Draw `gauge` in the bar-wide style.
pub fn render(m: &Meter, gauge: Gauge) -> String {
    let s = spec(m.style);
    let mut out = String::new();
    let cells = if s.fixed_cells > 0 { s.fixed_cells } else { m.cells };
    if cells == 0 {
        return out;
    }
    let steps = s.steps;
    let value = gauge.value.min(100);
    let severity = gauge.severity;

    // The tip caps the whole gauge, so it always says how alarming the
    // reading is, even when the fill is one neutral color.
    let tip_role = if severity >= m.crit_at {
        Role::Crit
    } else if severity >= m.warn_at {
        Role::Warn
    } else {
        Role::Ok
    };
    let (mut fill_role, mut edge_role) = (s.fill_role, s.edge_role);
    if s.tonal {
        fill_role = if s.painted {
            if severity >= m.crit_at {
                Role::BgCrit
            } else if severity >= m.warn_at {
                Role::BgWarn
            } else {
                Role::BgOk
            }
        } else {
            tip_role
        };
        edge_role = tip_role;
    }

    let marked = if s.plain { 0 } else { gauge.marked.min(value) };
    let total = (cells * steps * value + 50) / 100;
    let mut accent = if marked > 0 { (cells * steps * marked + 50) / 100 } else { 0 };
    // A share small enough to round away is still worth one sub-step.
    if marked > 0 && accent == 0 {
        accent = 1;
    }
    accent = accent.min(total);
    let plain = total - accent;

    let (whole, rem);
    if accent > 0 {
        // A cell cannot show fill, accent and trough at once, so the split
        // snaps to a cell edge. The total fill length stays exact.
        let mut w = (plain + steps / 2) / steps;
        if w * steps >= total {
            w = if total > 0 { (total - 1) / steps } else { 0 };
        }
        whole = w;
        accent = total - w * steps;
        rem = 0;
    } else {
        whole = total / steps;
        rem = total % steps;
    }

    if let Some(custom) = s.custom {
        out.push(fill_role.ch());
        custom(value, marked, &mut out);
        out.push(Role::Normal.ch());
        return out;
    }

    let mut used = 0;
    // The head is drawn outside the cell budget and always, even at zero, so
    // gauges after it do not shift when it appears.
    if let Some(head) = s.head {
        out.push(tip_role.ch());
        out.push_str(head);
    }
    if whole > 0 {
        out.push(fill_role.ch());
        for _ in 0..whole {
            out.push_str(s.fill);
            used += 1;
        }
    }
    if rem > 0 {
        if let Some(partial) = s.partial {
            out.push(edge_role.ch());
            out.push_str(partial[rem as usize]);
            used += 1;
        }
    }
    if accent > 0 {
        out.push(s.accent_role.ch());
        while accent >= steps {
            out.push_str(s.accent);
            accent -= steps;
            used += 1;
        }
        if accent > 0 {
            if let Some(partial) = s.partial {
                out.push_str(partial[accent as usize]);
                used += 1;
            }
        }
    }
    // A gauge with nothing filled has no tip: a tail on its own reads as a
    // reading rather than as the absence of one.
    if let Some(tip) = s.tip {
        if used > 0 && used < cells {
            out.push(tip_role.ch());
            out.push_str(tip);
            used += 1;
        }
    }
    out.push(s.trough_role.ch());
    while used < cells {
        out.push_str(s.trough);
        used += 1;
    }
    out.push(Role::Normal.ch());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visible(s: &str) -> String {
        s.chars().filter(|&c| c >= ' ').collect()
    }

    #[test]
    fn box_half_cell() {
        let m = Meter::default();
        // 6 cells * 2 steps * 42% = 5.04 -> 5 half-cells: two whole, one half.
        assert_eq!(visible(&render(&m, Gauge::new(42))), "━━╾───");
    }

    #[test]
    fn box_colors_by_severity() {
        let m = Meter::default();
        assert!(render(&m, Gauge::new(90)).contains(Role::Crit.ch()));
        assert!(render(&m, Gauge::new(70)).contains(Role::Warn.ch()));
        assert!(render(&m, Gauge::new(10)).contains(Role::Ok.ch()));
    }

    #[test]
    fn box_marked_share() {
        let m = Meter::default();
        let out = render(&m, Gauge { value: 50, marked: 20, severity: 50 });
        assert_eq!(visible(&out), "━━━───");
        assert!(out.contains(Role::Mark.ch()));
    }

    #[test]
    fn empty_and_full() {
        let m = Meter::default();
        assert_eq!(visible(&render(&m, Gauge::new(0))), "──────");
        assert_eq!(visible(&render(&m, Gauge::new(100))), "━━━━━━");
    }

    #[test]
    fn spark_is_one_cell() {
        let m = Meter { style: Style::Spark, ..Meter::default() };
        assert_eq!(visible(&render(&m, Gauge::new(50))), "▄");
    }
}
