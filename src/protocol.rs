//! What stanley writes for dwl's bar, and what dwl writes back.
//!
//! One line per tick. A block starts with `BLOCK` and an index byte; bytes
//! 0x01..=0x0f select a color role and take no width.
//!
//! Protocol 2 opens each of three sections with a byte of its own and names
//! roles through the table in docs/design.md. Protocol 1 is what slstatus
//! wrote: no sections, and a role's byte is dwl's `Scheme` index plus one.
//! Widgets always paint protocol 2 bytes; `to_v1` translates a finished line
//! for a dwl that predates the table.

/// Starts a block. dwl strips it and the index byte after it.
pub const BLOCK: char = '\x1f';

/// The index byte is offset into printable ASCII: a raw 10 would be a newline
/// and cut the line at the tenth block.
pub const INDEX_BASE: u32 = 0x20;

/// The most blocks the index byte can address while staying below DEL.
pub const MAX_BLOCKS: usize = 0x7e - INDEX_BASE as usize;

/// Open the left, center and right sections, in that order.
pub const LEFT: char = '\x1c';
pub const CENTER: char = '\x1d';
pub const RIGHT: char = '\x1e';

/// Which protocol dwl reads. stanley is piped into dwl, not started by it,
/// so the session asks `dwl -p` and passes the answer on in
/// `DWL_BAR_PROTOCOL`. Anything else, an old dwl included, means 1.
pub fn version() -> u32 {
    if std::env::var("DWL_BAR_PROTOCOL").is_ok_and(|v| v.trim() == "2") {
        2
    } else {
        1
    }
}

/// A color role. stanley says what a run of text means; dwl decides its color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The bar's own foreground.
    Normal,
    /// Red: an alert, or a reading past `crit-at`.
    Crit,
    /// Dim: off, idle, or a trough.
    Dim,
    /// Yellow: a reading past `warn-at`.
    Warn,
    /// Green: a reading below both thresholds.
    Ok,
    /// Purple: a marked share inside a gauge.
    Mark,
    /// Painted gauges only: fill on trough, fill as background, accent.
    FillEdge,
    FillBg,
    Accent,
    /// Painted gauges only: severity as a background.
    BgOk,
    BgWarn,
    BgCrit,
    /// Grey: secondary text.
    Muted,
}

impl Role {
    /// The protocol 2 selector byte. 0x0a is skipped: dwl reads the status
    /// a line at a time.
    pub fn ch(self) -> char {
        match self {
            Role::Normal => '\x01',
            Role::Crit => '\x02',
            Role::Dim => '\x03',
            Role::Warn => '\x04',
            Role::Ok => '\x05',
            Role::Mark => '\x06',
            Role::FillEdge => '\x07',
            Role::FillBg => '\x08',
            Role::Accent => '\x09',
            Role::BgOk => '\x0b',
            Role::BgWarn => '\x0c',
            Role::BgCrit => '\x0d',
            Role::Muted => '\x0e',
        }
    }
}

/// A protocol 2 line's role bytes as protocol 1 spells them. `FillBg` becomes
/// 0x0a, a newline, so the painted meter styles cannot work there; the box
/// style does not use it.
pub fn to_v1(line: &str) -> String {
    line.chars()
        .map(|c| match c {
            '\x02' => '\x04',
            '\x03' => '\x05',
            '\x04' => '\x06',
            '\x05' => '\x07',
            '\x06' => '\x08',
            '\x07' => '\x09',
            '\x08' => '\x0a',
            '\x09' => '\x0b',
            '\x0b' => '\x0c',
            '\x0c' => '\x0d',
            '\x0d' => '\x0e',
            '\x0e' => '\x0f',
            c => c,
        })
        .collect()
}

/// `text` in `role`, then back to the bar's foreground.
pub fn paint(role: Role, text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push(role.ch());
    out.push_str(text);
    out.push(Role::Normal.ch());
    out
}

/// Text from outside (a process name, a device description) with every
/// control byte replaced. Below 0x20 is where the role and block bytes live:
/// one stray byte would recolor the rest of the bar or misplace every click
/// to its right.
pub fn clean(text: &str) -> String {
    text.chars()
        .map(|c| if c < ' ' || c == '\x7f' { '?' } else { c })
        .collect()
}

/// Whether `text` would draw anything once its role bytes are dropped.
pub fn is_blank(text: &str) -> bool {
    text.chars().all(|c| c < ' ')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_replaces_control_bytes() {
        assert_eq!(clean("a\x1fb\x01c\x7f"), "a?b?c?");
    }

    #[test]
    fn paint_resets() {
        assert_eq!(paint(Role::Crit, "x"), "\x02x\x01");
    }

    #[test]
    fn v1_bytes_are_dwl_schemes_plus_one() {
        assert_eq!(to_v1("\x01a\x02b\x0ec\x1f!"), "\x01a\x04b\x0fc\x1f!");
    }

    #[test]
    fn blank_ignores_roles() {
        assert!(is_blank("\x05\x01"));
        assert!(!is_blank("\x05x"));
    }
}
