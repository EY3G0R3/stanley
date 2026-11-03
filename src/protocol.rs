//! What stanley writes for dwl's bar, and what dwl writes back.
//!
//! One line per tick. A block starts with `BLOCK` and an index byte; bytes
//! 0x01..=0x0f select a color role and take no width. Phase 1 keeps the
//! bytes slstatus used, so dwl needs no change: a role's byte is dwl's
//! `Scheme` index plus one (see `colors[]` in dwl's config.h).

/// Starts a block. dwl strips it and the index byte after it.
pub const BLOCK: char = '\x1f';

/// The index byte is offset into printable ASCII: a raw 10 would be a newline
/// and cut the line at the tenth block.
pub const INDEX_BASE: u32 = 0x20;

/// The most blocks the index byte can address while staying below DEL.
pub const MAX_BLOCKS: usize = 0x7e - INDEX_BASE as usize;

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
    /// The selector byte dwl reads.
    ///
    /// `FillBg` is 0x0a, a newline, and dwl reads the status a line at a
    /// time, so the painted styles that use it cannot work on dwl until the
    /// phase 2 role table moves it. The box style in use does not need it.
    pub fn ch(self) -> char {
        match self {
            Role::Normal => '\x01',
            Role::Crit => '\x04',
            Role::Dim => '\x05',
            Role::Warn => '\x06',
            Role::Ok => '\x07',
            Role::Mark => '\x08',
            Role::FillEdge => '\x09',
            Role::FillBg => '\x0a',
            Role::Accent => '\x0b',
            Role::BgOk => '\x0c',
            Role::BgWarn => '\x0d',
            Role::BgCrit => '\x0e',
            Role::Muted => '\x0f',
        }
    }
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
        assert_eq!(paint(Role::Crit, "x"), "\x04x\x01");
    }

    #[test]
    fn blank_ignores_roles() {
        assert!(is_blank("\x05\x01"));
        assert!(!is_blank("\x05x"));
    }
}
