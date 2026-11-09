//! The active keyboard layout, as dwl reports it; a click switches to the
//! next one.

use super::{icon, Reading, Block};
use crate::protocol::clean;
use std::fs;

pub struct Keyboard;

/// dwl keeps the layout's short code (`us`, `ru`) in
/// $XDG_RUNTIME_DIR/dwl-kblayout and replaces the file on every change, so
/// a plain read each tick sees the new one. A dwl without it leaves no file,
/// and the widget stays off the bar.
fn layout() -> Option<String> {
    let dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
    code(&fs::read_to_string(format!("{dir}/dwl-kblayout")).ok()?)
}

fn code(text: &str) -> Option<String> {
    let code = text.lines().next()?.trim();
    (!code.is_empty()).then(|| clean(&code.to_uppercase()))
}

impl Block for Keyboard {
    fn click(&mut self, button: u8) -> Option<String> {
        (button == 1).then(|| "\"$HOME/src/dwl/dwlc\" nextkblayout".into())
    }

    fn read(&mut self) -> Reading {
        let Some(code) = layout() else { return Reading::default() };
        Reading { icon: Some(icon::KEYBOARD.into()), name: Some("KB".into()), extra: Some(code), ..Reading::default() }
    }
}

#[cfg(test)]
mod tests {
    use super::code;

    #[test]
    fn code_is_the_first_line_in_upper_case() {
        assert_eq!(code("ua\n").as_deref(), Some("UA"));
        assert_eq!(code(" us \nru\n").as_deref(), Some("US"));
        assert_eq!(code(""), None);
        assert_eq!(code("\n"), None);
    }
}
