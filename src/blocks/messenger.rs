//! Messenger service state and its private, expiring outpost snapshot.
use super::{dim, popup, Block, Reading};
use crate::protocol::{paint, Role};
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use super::messenger_unread::{Target, Unread};

const UNIT: &str = "quiver-outpost-messenger.service";
const EVERY: Duration = Duration::from_secs(5);
const ICON: &str = "\u{f075}";

fn query() -> Command {
    let mut command = Command::new("systemctl");
    command.args(["--user", "show", "--property=LoadState,ActiveState,InvocationID", UNIT]);
    command
}

#[derive(Default)]
struct Service {
    state: String,
    invocation: String,
}

fn service(text: &str) -> Service {
    let field = |name: &str| text.lines().find_map(|l| l.strip_prefix(name)).unwrap_or("");
    if field("LoadState=") != "loaded" {
        return Service::default();
    }
    Service { state: field("ActiveState=").into(), invocation: field("InvocationID=").into() }
}

/// A snapshot is valid only for this service invocation and a recent heartbeat.
/// Never infer unread from chats, timestamps, or duplicated breakout messages.
fn snapshot(text: &str, invocation: &str, now: u64) -> Option<(String, Option<u64>)> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let stamp = v["updatedAt"].as_u64()?;
    if v["version"].as_u64()? != 1 || invocation.is_empty()
        || v["invocationId"].as_str()? != invocation || stamp > now || now - stamp > 30
    {
        return None;
    }
    let connection = v["connection"].as_str()?;
    if !matches!(connection, "online" | "offline" | "connecting" | "error") {
        return None;
    }
    let source_known = matches!(v["unreadSource"].as_str(), Some("messenger" | "quiver"));
    let count = if connection == "online" && source_known { v["unreadMessages"].as_u64() } else { None };
    Some((connection.into(), count))
}

fn reading(s: &Service, data: Option<(String, Option<u64>)>) -> Reading {
    let (text, muted, bad) = match s.state.as_str() {
        "inactive" => ("off".into(), true, false),
        "failed" => ("failed ?".into(), false, true),
        "activating" => ("starting ?".into(), true, false),
        "deactivating" => ("stopping ?".into(), true, false),
        "active" => match data {
            Some((connection, count)) if connection == "online" =>
                (count.map_or_else(|| "online ?".into(), |n| format!("{n}")), false, false),
            Some((connection, _)) => (format!("{connection} ?"), false, connection == "error"),
            None => ("running ?".into(), true, false),
        },
        _ => ("unavailable ?".into(), true, false),
    };
    Reading {
        icon: Some(if muted { dim(ICON) } else { ICON.into() }),
        name: Some("Messenger".into()),
        extra: Some(if bad { paint(Role::Crit, &text) } else if muted { dim(&text) } else { text }),
        ..Reading::default()
    }
}

pub struct Messenger {
    running: Option<(Child, Instant)>,
    next: Instant,
    service: Service,
    unread: Unread,
}

impl Messenger {
    pub fn new() -> Self {
        Self { running: None, next: Instant::now(), service: Service::default(), unread: Unread::new() }
    }
}

impl Block for Messenger {
    fn click(&mut self, button: u8) -> Option<String> { popup(button, "messenger") }

    fn read(&mut self) -> Reading {
        if let Some((child, started)) = self.running.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let mut output = String::new();
                    if status.success() {
                        if let Some(out) = child.stdout.take() {
                            let _ = out.take(8192).read_to_string(&mut output);
                        }
                    }
                    self.service = service(&output);
                    self.running = None;
                }
                Ok(None) if started.elapsed() < Duration::from_secs(3) => {}
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    self.running = None;
                    self.service = Service::default();
                }
            }
        }
        if self.running.is_none() && Instant::now() >= self.next {
            self.next = Instant::now() + EVERY;
            match query()
                .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
            {
                Ok(child) => self.running = Some((child, Instant::now())),
                Err(_) => self.service = Service::default(),
            }
        }
        let mut target = None;
        let data = std::env::var_os("XDG_RUNTIME_DIR").and_then(|dir| {
            let path = std::path::PathBuf::from(dir).join("quiver-outpost-messenger/status.json");
            let mut text = String::new();
            std::fs::File::open(path).ok()?.take(8192).read_to_string(&mut text).ok()?;
            let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
            let data = snapshot(&text, &self.service.invocation, now)?;
            if self.service.state == "active" && data.0 == "online" && data.1.is_none() {
                target = serde_json::from_str(&text).ok().and_then(|v| Target::from_snapshot(&v));
            }
            Some(data)
        });
        let fallback = self.unread.read(target);
        let data = data.map(|(connection, count)| (connection, count.or(fallback)));
        reading(&self.service, data)
    }
}

impl Drop for Messenger {
    fn drop(&mut self) {
        if let Some((child, _)) = self.running.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(connection: &str, count: &str) -> String {
        format!(r#"{{"version":1,"updatedAt":100,"invocationId":"abc","connection":"{connection}","unreadSource":"messenger","unreadMessages":{count}}}"#)
    }

    #[test]
    fn unknown_is_never_zero() {
        let active = Service { state: "active".into(), invocation: "abc".into() };
        assert!(reading(&active, None).extra.unwrap().contains("running ?"));
        assert_eq!(reading(&active, snapshot(&source("online", "0"), "abc", 100)).extra.as_deref(), Some("0"));
        assert_eq!(reading(&active, snapshot(&source("online", "12"), "abc", 100)).extra.as_deref(), Some("12"));
        for count in ["null", "-1", "1.5", "\"0\""] {
            assert!(reading(&active, snapshot(&source("online", count), "abc", 100)).extra.unwrap().contains('?'));
        }
    }

    #[test]
    fn reject_stale_foreign_malformed_and_offline_counts() {
        assert!(snapshot(&source("online", "4"), "abc", 131).is_none());
        assert!(snapshot(&source("online", "4"), "abc", 99).is_none());
        assert!(snapshot(&source("online", "4"), "def", 100).is_none());
        assert!(snapshot("{}", "abc", 100).is_none());
        for state in ["offline", "connecting", "error"] {
            assert_eq!(snapshot(&source(state, "4"), "abc", 100).unwrap().1, None);
        }
    }

    #[test]
    fn service_states_and_click() {
        for (state, expected) in [("inactive", "off"), ("failed", "failed ?"), ("activating", "starting ?"),
            ("deactivating", "stopping ?"), ("", "unavailable ?")] {
            let s = service(&format!("LoadState=loaded\nActiveState={state}\nInvocationID=abc\n"));
            assert!(reading(&s, Some(("online".into(), Some(7)))).extra.unwrap().contains(expected));
        }
        assert!(service("LoadState=not-found\nActiveState=inactive").state.is_empty());
        let mut block = Messenger::new();
        assert_eq!(block.click(1).as_deref(), Some("stanley-popup messenger"));
        assert_eq!(block.click(3), None);
        assert_eq!(query().get_args().collect::<Vec<_>>(),
            ["--user", "show", "--property=LoadState,ActiveState,InvocationID", UNIT]);
    }

    #[test]
    fn configured_widget_renders_state_and_count() {
        let mut problems = Vec::new();
        let config = crate::config::parse("messenger icon extra\n", &mut |_, e| problems.push(e.to_string()));
        assert!(problems.is_empty());
        assert!(super::super::Factory::new().build("messenger", None).is_ok());
        let active = Service { state: "active".into(), invocation: "abc".into() };
        for (data, expected) in [(None, "running ?"), (Some(("online".into(), Some(0))), "0"),
            (Some(("online".into(), Some(8))), "8"), (Some(("offline".into(), None)), "offline ?"),
            (Some(("error".into(), None)), "error ?")] {
            let (text, bare) = crate::render::block(config.entries[0].parts, &reading(&active, data), &config.settings);
            let visible: String = text.chars().filter(|&c| c >= ' ').collect();
            assert_eq!(visible, format!("{ICON} {expected}"));
            assert!(!bare);
        }
    }
}
