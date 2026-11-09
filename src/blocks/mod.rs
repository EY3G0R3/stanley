//! Blocks: what sits on the bar, one file each. A block only reads; `render`
//! decides what shows. The bar file names them as widgets.

mod audio;
mod battery;
mod clock;
mod cpu;
mod disk;
mod fixer;
mod hostname;
mod keyboard;
mod memory;
mod recording;
mod script;
mod top;

use crate::meter::Gauge;
use crate::protocol::paint;
use crate::protocol::Role;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

/// What a widget has to show. `icon`, `name` and `extra` may carry color
/// roles (see `protocol`); anything taken from outside must go through
/// `protocol::clean` first.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reading {
    pub icon: Option<String>,
    pub name: Option<String>,
    pub gauge: Option<Gauge>,
    pub extra: Option<String>,
}

pub trait Block {
    /// Take a fresh reading.
    fn read(&mut self) -> Reading;

    /// How often to read. `None`: every tick.
    fn interval(&self) -> Option<Duration> {
        None
    }

    /// What a click does. `button` is 1 left, 2 middle, 3 right, 4 and 5 a
    /// wheel notch. Returns a shell command to run, or None when the block
    /// did it itself or ignores that button. A bar-file `click:` overrides
    /// this.
    fn click(&mut self, _button: u8) -> Option<String> {
        None
    }
}

/// Open popup `name` (see `src/popups/`) on a left click.
fn popup(button: u8, name: &str) -> Option<String> {
    (button == 1).then(|| format!("stanley-popup {name}"))
}

/// Icons, all checked against the bar's font. Most are in the private use
/// area, so they are spelled as escapes: editors and pipes drop them silently.
pub mod icon {
    pub const CPU: &str = "\u{f2db}"; // a processor
    pub const RAM: &str = "\u{e266}"; // memory modules (U+F538 is missing from the font)
    pub const DISK: &str = "\u{f1c0}"; // stacked platters
    pub const TMP: &str = "\u{f0aba}"; // a folder with a clock
    pub const SPEAKER: &str = "\u{f028}";
    pub const HDMI: &str = "\u{f108}"; // a monitor
    pub const PHONES: &str = "\u{f025}"; // headphones
    pub const BLUETOOTH: &str = "\u{f293}";
    pub const WEBCAM: &str = "\u{f030}";
    pub const MIC: &str = "\u{f130}";
    pub const AGENTS: &str = "\u{f06a9}"; // a robot
    pub const FIXER: &str = "\u{f04c6}"; // spellcheck: fix-my-messages
    pub const RECORD: &str = "\u{f0ec2}"; // as on the Omarchy bar
    pub const WIFI: &str = "\u{f1eb}";
    pub const PLUG: &str = "\u{f1e6}";
    pub const BATTERY: &str = "\u{f240}";
    pub const POWER: &str = "\u{f011}";
    pub const CLOCK: &str = "\u{f017}";
    pub const CALENDAR: &str = "\u{f073}";
    pub const HOST: &str = "\u{f109}"; // a laptop
    pub const KVM: &str = "\u{f1483}"; // a monitor with an arrow: hand it over
    pub const KEYBOARD: &str = "\u{f11c}";
}

/// An icon whose click opens a popup that shows the live state, so the icon
/// itself never changes.
struct Fixed {
    icon: &'static str,
    name: &'static str,
    popup: &'static str,
}

impl Block for Fixed {
    fn click(&mut self, button: u8) -> Option<String> {
        popup(button, self.popup)
    }

    fn read(&mut self) -> Reading {
        Reading { icon: Some(self.icon.into()), name: Some(self.name.into()), ..Reading::default() }
    }
}

/// The widgets a bar file can name, for error messages.
pub const NAMES: &[&str] = &[
    "top", "cpu", "memory", "disk", "tmp", "audio", "mic", "keyboard", "record", "agents", "messages",
    "bluetooth", "wifi", "battery", "kvm", "clock", "date", "hostname", "script",
];

/// Build every widget the bar file lists, and nothing else. Widgets that share
/// a source (audio and mic) share one connection to it.
pub struct Factory {
    pulse: Option<Rc<RefCell<audio::Pulse>>>,
}

impl Factory {
    pub fn new() -> Factory {
        Factory { pulse: None }
    }

    pub fn build(&mut self, widget: &str, arg: Option<&str>) -> Result<Box<dyn Block>, String> {
        Ok(match widget {
            "top" => Box::new(top::Top::new()),
            "cpu" => Box::new(cpu::Cpu::new()),
            "memory" => Box::new(memory::Memory),
            "disk" => Box::new(disk::Disk::new(arg.unwrap_or("/"), false)),
            "tmp" => Box::new(disk::Disk::new(arg.unwrap_or("/tmp"), true)),
            "audio" => Box::new(audio::Audio::new(self.pulse())),
            "mic" => Box::new(audio::Mic::new(self.pulse())),
            "keyboard" => Box::new(keyboard::Keyboard),
            "record" => Box::new(recording::Recording),
            "messages" => Box::new(fixer::Fixer),
            "battery" => Box::new(battery::Battery),
            "clock" => Box::new(clock::Clock::time()),
            "date" => Box::new(clock::Clock::date()),
            "hostname" => Box::new(hostname::Hostname),
            "script" => match arg {
                Some(command) => Box::new(script::Script::new(command)),
                None => return Err("script needs a command: script:~/bin/vpn-status".into()),
            },
            "agents" => Box::new(Fixed { icon: icon::AGENTS, name: "AI", popup: "agents" }),
            "bluetooth" => Box::new(Fixed { icon: icon::BLUETOOTH, name: "BT", popup: "bluetooth" }),
            "wifi" => Box::new(Fixed { icon: icon::WIFI, name: "WIFI", popup: "network" }),
            "kvm" => Box::new(Fixed { icon: icon::KVM, name: "KVM", popup: "kvm" }),
            _ => return Err(format!("unknown widget {widget:?}; known: {}", NAMES.join(", "))),
        })
    }

    fn pulse(&mut self) -> Rc<RefCell<audio::Pulse>> {
        self.pulse.get_or_insert_with(|| Rc::new(RefCell::new(audio::Pulse::new()))).clone()
    }
}

/// A widget that is off, idle or waiting is drawn dim.
fn dim(text: &str) -> String {
    paint(Role::Dim, text)
}
