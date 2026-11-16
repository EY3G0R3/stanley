//! A bounded, read-only human-session query. The outpost app token cannot
//! speak for the owner's Quiver read position.
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::os::unix::process::CommandExt;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Target {
    pub quiver: String,
    pub channel: String,
    pub owner: String,
    pub owner_id: String,
    pub invocation: String,
}

impl Target {
    pub fn from_snapshot(v: &serde_json::Value) -> Option<Self> {
        let quiver = v["quiverUrl"].as_str()?;
        let host = quiver.strip_prefix("https://").or_else(|| quiver.strip_prefix("http://"))?;
        if host.is_empty() || host.len() > 255
            || !host.chars().all(|c| c.is_ascii_alphanumeric() || ".-:".contains(c))
            || v["appId"].as_str()? != "quiver-messenger"
        {
            return None;
        }
        // --as takes an address without a scheme. Refuse an origin whose
        // scheme would change under the CLI's DNS/localhost resolver.
        let hostname = host.split(':').next()?;
        let dns = hostname.contains('.') && !hostname.chars().all(|c| c.is_ascii_digit() || c == '.');
        if quiver.starts_with("https://") != dns || host.matches(':').count() > 1 {
            return None;
        }
        let channel = v["channelId"].as_str()?;
        let owner = v["ownerHandle"].as_str()?;
        let owner_id = v["ownerMemberId"].as_str()?;
        if channel.is_empty() || channel.len() > 255 || !channel.chars().all(|c| c.is_ascii_alphanumeric())
            || owner.is_empty() || owner.len() > 255
            || !owner.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
            || owner_id.is_empty() || owner_id.len() > 255
            || !owner_id.chars().all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
        {
            return None;
        }
        Some(Self { quiver: host.into(), channel: channel.into(), owner: owner.into(), owner_id: owner_id.into(),
            invocation: v["invocationId"].as_str()?.into() })
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new("timeout");
        cmd.args(["--kill-after=1s", "8s", "quiver", "--as", &format!("{}@{}", self.owner, self.quiver),
            "messages", "app-unread", "--channel-id", &self.channel, "--app", "quiver-messenger",
            "--key-prefix", "in:", "--member-id", &self.owner_id, "--json"]);
        cmd.process_group(0);
        cmd
    }

    fn count(&self, output: &str) -> Option<u64> {
        let v: serde_json::Value = serde_json::from_str(output).ok()?;
        if v["memberHandle"].as_str()? != self.owner { return None; }
        v["unreadMessages"].as_u64()
    }
}

pub(super) struct Unread {
    target: Option<Target>,
    running: Option<(Child, Instant)>,
    next: Instant,
    value: Option<(u64, Instant)>,
}

impl Unread {
    pub fn new() -> Self {
        Self { target: None, running: None, next: Instant::now(), value: None }
    }

    fn cancel(&mut self) {
        if let Some((mut child, _)) = self.running.take() {
            // timeout owns a process group; cancel its CLI descendants too.
            unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL); }
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    pub fn read(&mut self, target: Option<Target>) -> Option<u64> {
        if self.target != target {
            self.cancel();
            self.target = target;
            self.value = None;
            self.next = Instant::now();
        }
        let target = self.target.as_ref()?.clone();
        if let Some((child, started)) = self.running.as_mut() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let mut output = String::new();
                    if status.success() {
                        if let Some(out) = child.stdout.take() {
                            let _ = out.take(8192).read_to_string(&mut output);
                        }
                    }
                    self.value = target.count(&output).map(|n| (n, Instant::now()));
                    self.running = None;
                }
                Ok(None) if started.elapsed() < Duration::from_secs(10) => {}
                _ => {
                    self.cancel();
                    self.value = None;
                }
            }
        }
        if self.running.is_none() && Instant::now() >= self.next {
            self.next = Instant::now() + Duration::from_secs(15);
            match target.command().stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn() {
                Ok(child) => self.running = Some((child, Instant::now())),
                Err(_) => self.value = None,
            }
        }
        self.value.filter(|(_, at)| at.elapsed() <= Duration::from_secs(20)).map(|(n, _)| n)
    }
}

impl Drop for Unread {
    fn drop(&mut self) { self.cancel(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> Target {
        Target::from_snapshot(&serde_json::json!({"quiverUrl":"https://example.test",
            "channelId":"abc123", "appId":"quiver-messenger", "ownerHandle":"owner", "ownerMemberId":"human123", "invocationId":"inv"})).unwrap()
    }

    #[test]
    fn query_is_scoped_read_only_and_requires_the_owner() {
        let t = target();
        assert_eq!(t.command().get_args().collect::<Vec<_>>(), ["--kill-after=1s", "8s", "quiver", "--as", "owner@example.test",
            "messages", "app-unread", "--channel-id", "abc123", "--app", "quiver-messenger", "--key-prefix", "in:",
            "--member-id", "human123", "--json"]);
        assert_eq!(t.count(r#"{"memberHandle":"owner","unreadMessages":0}"#), Some(0));
        assert_eq!(t.count(r#"{"memberHandle":"owner","unreadMessages":8}"#), Some(8));
        for bad in [r#"{"memberHandle":"other","unreadMessages":8}"#,
            r#"{"memberHandle":"owner","unreadMessages":null}"#,
            r#"{"memberHandle":"owner","unreadMessages":-1}"#, "{}", "not json"] {
            assert_eq!(t.count(bad), None);
        }
    }

    #[test]
    fn reject_missing_or_untrusted_context() {
        assert!(Target::from_snapshot(&serde_json::json!({})).is_none());
        for origin in ["https://user:secret@example.test", "https://example.test/path", "https://example.test?secret",
            "file:///tmp/x", "http://example.test", "https://127.0.0.1:8080"] {
            let mut v = serde_json::json!({"quiverUrl":origin, "channelId":"abc123", "appId":"quiver-messenger",
                "ownerHandle":"owner", "ownerMemberId":"human123", "invocationId":"inv"});
            assert!(Target::from_snapshot(&v).is_none());
            v["quiverUrl"] = "https://example.test".into();
            v["channelId"] = "--bad".into();
            assert!(Target::from_snapshot(&v).is_none());
        }
    }

    #[test]
    fn offline_and_expired_results_are_not_zero() {
        let mut unread = Unread::new();
        unread.value = Some((4, Instant::now()));
        assert_eq!(unread.read(None), None);
        unread.target = Some(target());
        unread.value = Some((4, Instant::now()));
        assert_eq!(unread.read(None), None);
        assert!(unread.value.is_none());
    }
}
