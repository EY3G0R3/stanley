//! The time and the date, as dwl drew them before phase 2.

use super::{icon, Reading, Block};
use std::ffi::CStr;

/// The local time in a strftime format. `clock` and `date` differ only in
/// their format and icon.
pub struct Clock {
    format: &'static CStr,
    icon: &'static str,
}

impl Clock {
    pub fn time() -> Clock {
        Clock { format: c"%I:%M %p", icon: icon::CLOCK }
    }

    pub fn date() -> Clock {
        Clock { format: c"%Y-%m-%d", icon: icon::CALENDAR }
    }
}

fn now(format: &CStr) -> Option<String> {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let mut buf = [0u8; 64];
    let n = unsafe {
        let t = libc::time(std::ptr::null_mut());
        if libc::localtime_r(&t, &mut tm).is_null() {
            return None;
        }
        libc::strftime(buf.as_mut_ptr().cast(), buf.len(), format.as_ptr(), &tm)
    };
    (n > 0).then(|| String::from_utf8_lossy(&buf[..n]).into_owned())
}

impl Block for Clock {
    fn click(&mut self, button: u8) -> Option<String> {
        (button == 1).then(|| {
            "yad --calendar --mouse --on-top --undecorated --skip-taskbar --fixed \
             --close-on-unfocus --no-buttons --width=280 --height=240 --title=Calendar"
                .into()
        })
    }

    fn read(&mut self) -> Reading {
        Reading { icon: Some(self.icon.into()), extra: now(self.format), ..Reading::default() }
    }
}
