//! The audio output (which card, what volume) and whether a microphone is
//! live, from PulseAudio or PipeWire's Pulse server.
//!
//! One connection stays open for the life of the bar and is shared by the
//! `audio` and `mic` widgets; it is opened only if one of them is on the bar.

use super::{icon, popup, Block, Reading};
use crate::meter::Gauge;
use crate::protocol::{paint, Role};
use libpulse_binding as pulse;
use pulse::callbacks::ListResult;
use pulse::context::{Context, FlagSet, State};
use pulse::mainloop::standard::Mainloop;
use pulse::operation::State as OpState;
use pulse::proplist::properties::DEVICE_DESCRIPTION;
use pulse::time::MicroSeconds;
use pulse::volume::{ChannelVolumes, Volume};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// Readings this recent are reused, so `audio` and `mic` share one query.
const FRESH: Duration = Duration::from_millis(500);
/// A Pulse server that does not answer within this is treated as gone.
const TIMEOUT: Duration = Duration::from_secs(1);

/// What the bar calls each card, by its device description. Several
/// Bluetooth headsets share one icon: at a glance what matters is that sound
/// goes to the headset rather than to the monitor.
const DEVICES: &[(&str, &str)] = &[
    ("GM204 High Definition Audio Controller", "HDMI"),
    ("HDA NVidia", "HDMI"),
    ("AD104 High Definition Audio Controller", "HDMI"),
    ("AD107 High Definition Audio Controller", "HDMI"),
    ("Meteor Lake-P HD Audio Controller", "HDMI"),
    ("USB PnP Audio Device", "HEADPHONES"),
    ("Built-in Audio", "HEADPHONES"),
    ("i7mini", "BLUETOOTH"),
    ("Aeropex by AfterShokz", "AEROPEX"),
    ("OpenRun by Shokz", "SHOKZ"),
    ("WH-XB910N", "SONY"),
    ("UVC Camera", "WEBCAM"),
    ("LifeCam HD-3000", "WEBCAM"),
];

fn display_name(description: &str) -> &'static str {
    DEVICES.iter().find(|(d, _)| *d == description).map_or("UNKNOWN", |(_, n)| n)
}

fn card_icon(label: &str) -> &'static str {
    match label.split('+').next().unwrap_or("") {
        "HDMI" => icon::HDMI,
        "HEADPHONES" => icon::PHONES,
        "BLUETOOTH" | "AEROPEX" | "SHOKZ" | "SONY" => icon::BLUETOOTH,
        "WEBCAM" => icon::WEBCAM,
        _ => icon::SPEAKER,
    }
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    /// The output's card, e.g. "SHOKZ"; "MUTE" when no card is on.
    pub label: String,
    /// 0..=100, 0 when muted.
    pub volume: u32,
    pub mic_live: bool,
}

/// Field order matters: Rust drops fields in declaration order, and the
/// context points into the main loop, so it has to go first.
struct Connection {
    context: Context,
    mainloop: Mainloop,
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.context.disconnect();
    }
}

pub struct Pulse {
    connection: Option<Connection>,
    snapshot: Option<Snapshot>,
    taken: Option<Instant>,
}

impl Pulse {
    pub fn new() -> Pulse {
        Pulse { connection: None, snapshot: None, taken: None }
    }

    /// The current state, or None when the server cannot be reached.
    pub fn snapshot(&mut self) -> Option<Snapshot> {
        if self.taken.is_some_and(|t| t.elapsed() < FRESH) {
            return self.snapshot.clone();
        }
        self.taken = Some(Instant::now());
        self.snapshot = self.query();
        if self.snapshot.is_none() {
            // Reconnect next time: the server may have restarted.
            self.connection = None;
        }
        self.snapshot.clone()
    }

    fn query(&mut self) -> Option<Snapshot> {
        if self.connection.is_none() {
            self.connection = connect();
        }
        let conn = self.connection.as_mut()?;
        if conn.context.get_state() != State::Ready {
            return None;
        }
        let deadline = Instant::now() + TIMEOUT;

        let default_sink = Rc::new(RefCell::new(None::<String>));
        let sink = default_sink.clone();
        let op = conn.context.introspect().get_server_info(move |info| {
            *sink.borrow_mut() = info.default_sink_name.as_ref().map(|n| n.to_string());
        });
        pump(&mut conn.mainloop, deadline, || op.get_state() != OpState::Running)?;
        let default_sink = default_sink.borrow().clone();

        #[derive(Default)]
        struct Found {
            cards: Vec<(u32, &'static str)>,
            mic_live: bool,
            sink: Option<(u32, Option<u32>)>, // (volume, card index)
            failed: bool,
        }
        let found = Rc::new(RefCell::new(Found::default()));

        let f = found.clone();
        let cards = conn.context.introspect().get_card_info_list(move |result| match result {
            ListResult::Item(card) => {
                let Some(profile) = card.active_profile.as_ref().and_then(|p| p.name.as_deref()) else {
                    return;
                };
                if profile == "off" {
                    return;
                }
                let mut f = f.borrow_mut();
                // The mic is off at the card-profile level unless in use.
                if profile == "input:analog-stereo" || profile.contains("+input:analog-stereo") {
                    f.mic_live = true;
                }
                let name = display_name(&card.proplist.get_str(DEVICE_DESCRIPTION).unwrap_or_default());
                if name != "WEBCAM" {
                    f.cards.push((card.index, name));
                }
            }
            ListResult::End => {}
            ListResult::Error => f.borrow_mut().failed = true,
        });

        let f = found.clone();
        let sink = default_sink.map(|name| {
            conn.context.introspect().get_sink_info_by_name(&name, move |result| match result {
                ListResult::Item(info) => {
                    let average = info.volume.avg().0 as u64;
                    let volume = if info.mute {
                        0
                    } else {
                        ((average * 100 + Volume::NORMAL.0 as u64 / 2) / Volume::NORMAL.0 as u64) as u32
                    };
                    f.borrow_mut().sink = Some((volume.min(100), info.card));
                }
                ListResult::End => {}
                ListResult::Error => f.borrow_mut().failed = true,
            })
        });

        pump(&mut conn.mainloop, deadline, || {
            cards.get_state() != OpState::Running
                && sink.as_ref().is_none_or(|s| s.get_state() != OpState::Running)
        })?;

        let found = found.borrow();
        if found.failed {
            return None;
        }
        let (volume, sink_card) = found.sink.unwrap_or((0, None));
        // Under X, igoraudio-set-card turned every other card off, so the one
        // active card was the output. Under dwl, WirePlumber only moves the
        // default sink, so name that sink's card when there is one.
        let label = match found.cards.iter().find(|(index, _)| Some(*index) == sink_card) {
            Some((_, name)) => name.to_string(),
            None if found.cards.is_empty() => "MUTE".to_string(),
            None => found.cards.iter().map(|(_, n)| *n).collect::<Vec<_>>().join("+"),
        };
        Some(Snapshot { label, volume, mic_live: found.mic_live })
    }
}

/// What a click changes on the default output.
pub enum Change {
    Mute,
    Up,
    Down,
}

impl Pulse {
    /// Toggle mute, or move the volume 2 % (up to 100 %), on the default
    /// output, through the connection the bar already holds. The next read
    /// queries afresh, so the gauge shows the change on the redraw that the
    /// click triggers.
    pub fn change(&mut self, change: Change) {
        self.taken = None;
        if self.connection.is_none() {
            self.connection = connect();
        }
        let Some(conn) = self.connection.as_mut() else { return };
        let deadline = Instant::now() + TIMEOUT;

        let sink = Rc::new(RefCell::new(None::<String>));
        let s = sink.clone();
        let op = conn.context.introspect().get_server_info(move |info| {
            *s.borrow_mut() = info.default_sink_name.as_ref().map(|n| n.to_string());
        });
        if pump(&mut conn.mainloop, deadline, || op.get_state() != OpState::Running).is_none() {
            return;
        }
        let Some(name) = sink.borrow().clone() else { return };

        let state = Rc::new(RefCell::new(None::<(ChannelVolumes, bool)>));
        let s = state.clone();
        let op = conn.context.introspect().get_sink_info_by_name(&name, move |result| {
            if let ListResult::Item(info) = result {
                *s.borrow_mut() = Some((info.volume, info.mute));
            }
        });
        if pump(&mut conn.mainloop, deadline, || op.get_state() != OpState::Running).is_none() {
            return;
        }
        let Some((mut volume, mute)) = *state.borrow() else { return };

        let step = Volume(Volume::NORMAL.0 / 50);
        let mut introspect = conn.context.introspect();
        let op = match change {
            Change::Mute => introspect.set_sink_mute_by_name(&name, !mute, None),
            Change::Up => {
                volume.inc_clamp(step, Volume::NORMAL);
                introspect.set_sink_volume_by_name(&name, &volume, None)
            }
            Change::Down => {
                volume.decrease(step);
                introspect.set_sink_volume_by_name(&name, &volume, None)
            }
        };
        let _ = pump(&mut conn.mainloop, deadline, || op.get_state() != OpState::Running);
    }
}

/// The clicks the audio and mic blocks share: left opens the popup, middle
/// mutes, the wheel moves the volume.
fn click(pulse: &RefCell<Pulse>, button: u8) -> Option<String> {
    let change = match button {
        2 => Change::Mute,
        4 => Change::Up,
        5 => Change::Down,
        _ => return popup(button, "audio"),
    };
    pulse.borrow_mut().change(change);
    None
}

fn connect() -> Option<Connection> {
    let mut mainloop = Mainloop::new()?;
    let mut context = Context::new(&mainloop, "stanley")?;
    context.connect(None, FlagSet::NOFLAGS, None).ok()?;
    let deadline = Instant::now() + TIMEOUT;
    pump(&mut mainloop, deadline, || matches!(context.get_state(), State::Ready | State::Failed | State::Terminated))?;
    (context.get_state() == State::Ready).then_some(Connection { mainloop, context })
}

/// Run the main loop until `done`, giving up at `deadline`. Bounded, so a
/// hung server cannot freeze the bar.
fn pump(mainloop: &mut Mainloop, deadline: Instant, mut done: impl FnMut() -> bool) -> Option<()> {
    while !done() {
        let left = deadline.checked_duration_since(Instant::now())?;
        mainloop.prepare(Some(MicroSeconds(left.as_micros().min(100_000) as u64))).ok()?;
        mainloop.poll().ok()?;
        mainloop.dispatch().ok()?;
    }
    Some(())
}

pub struct Audio {
    pulse: Rc<RefCell<Pulse>>,
}

impl Audio {
    pub fn new(pulse: Rc<RefCell<Pulse>>) -> Audio {
        Audio { pulse }
    }
}

impl Block for Audio {
    fn click(&mut self, button: u8) -> Option<String> {
        click(&self.pulse, button)
    }

    fn read(&mut self) -> Reading {
        match self.pulse.borrow_mut().snapshot() {
            Some(s) => Reading {
                icon: Some(card_icon(&s.label).into()),
                name: Some(s.label.clone()),
                gauge: Some(Gauge::new(s.volume)),
                extra: None,
            },
            None => Reading {
                icon: Some(paint(Role::Dim, icon::SPEAKER)),
                name: Some(paint(Role::Dim, "n/a")),
                ..Reading::default()
            },
        }
    }
}

/// Empty almost always: it appears only while a microphone is live, and in
/// red, as an alert rather than as another gauge.
pub struct Mic {
    pulse: Rc<RefCell<Pulse>>,
}

impl Mic {
    pub fn new(pulse: Rc<RefCell<Pulse>>) -> Mic {
        Mic { pulse }
    }
}

impl Block for Mic {
    fn click(&mut self, button: u8) -> Option<String> {
        click(&self.pulse, button)
    }

    fn read(&mut self) -> Reading {
        if !self.pulse.borrow_mut().snapshot().is_some_and(|s| s.mic_live) {
            return Reading::default();
        }
        Reading {
            icon: Some(paint(Role::Crit, icon::MIC)),
            name: Some(paint(Role::Crit, "MIC LIVE")),
            ..Reading::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_follow_the_first_card() {
        assert_eq!(card_icon("SHOKZ"), icon::BLUETOOTH);
        assert_eq!(card_icon("HDMI+SHOKZ"), icon::HDMI);
        assert_eq!(card_icon("UNKNOWN"), icon::SPEAKER);
    }

    #[test]
    fn unknown_devices() {
        assert_eq!(display_name("AD104 High Definition Audio Controller"), "HDMI");
        assert_eq!(display_name("Something New"), "UNKNOWN");
    }
}
