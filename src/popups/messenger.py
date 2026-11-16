"""The Messenger outpost: service controls and an expiring status snapshot."""
import json
import os
import time

from common import Panel, button, clear, label, run_async

UNIT = "quiver-outpost-messenger.service"
SHOW = ["systemctl", "--user", "show", "--property=LoadState,ActiveState,InvocationID", UNIT]


def query_target(data):
    """Bind a saved human session to this outpost's acknowledged installation."""
    try:
        if data["connection"] != "online" or data["appId"] != "quiver-messenger":
            return None
        origin = data["quiverUrl"]
        scheme, address = origin.split("://", 1)
        if (scheme not in ("http", "https") or not 0 < len(address) <= 255
                or not all(c.isascii() and (c.isalnum() or c in ".-:") for c in address)):
            return None
        hostname = address.split(":", 1)[0]
        dns = "." in hostname and not all(c.isdigit() or c == "." for c in hostname)
        if (scheme == "https") != dns or address.count(":") > 1:
            return None
        channel, owner, member = data["channelId"], data["ownerHandle"], data["ownerMemberId"]
        for value, extra in ((channel, ""), (owner, "-_."), (member, "-_")):
            if (not isinstance(value, str) or not 0 < len(value) <= 255
                    or not all(c.isascii() and (c.isalnum() or c in extra) for c in value)):
                return None
        return address, channel, owner, member, data["invocationId"]
    except (AttributeError, KeyError, TypeError, ValueError):
        return None


def query_command(target):
    address, channel, owner, member, _invocation = target
    return ["timeout", "--kill-after=1s", "8s", "quiver", "--as", f"{owner}@{address}",
            "messages", "app-unread", "--channel-id", channel, "--app", "quiver-messenger",
            "--key-prefix", "in:", "--member-id", member, "--json"]


def query_result(ok, output, target):
    try:
        value = json.loads(output)
        if ok and value["memberHandle"] == target[2]:
            count = value.get("unreadMessages")
            reason = value.get("unreadReason")
            return (count if type(count) is int and 0 <= count <= 2**64 - 1 else None,
                    reason[:240] if isinstance(reason, str) else "Quiver unread is unavailable")
    except (KeyError, TypeError, ValueError):
        pass
    return None, "Could not query the owner's saved Quiver session"


def snapshot(invocation, now=None):
    """Read no credentials or messages. A previous service run is never current."""
    try:
        runtime = os.environ["XDG_RUNTIME_DIR"]
        with open(os.path.join(runtime, "quiver-outpost-messenger", "status.json")) as f:
            value = json.loads(f.read(8192))
        stamp = value["updatedAt"]
        age = (time.time() if now is None else now) - stamp
        if (type(value["version"]) is not int or value["version"] != 1
                or type(stamp) is not int or not 0 <= age <= 30
                or not invocation or value["invocationId"] != invocation
                or value["connection"] not in ("online", "offline", "connecting", "error")):
            return None
        return value
    except (OSError, KeyError, ValueError, TypeError):
        return None


def describe(ok, output, source):
    fields = dict(line.split("=", 1) for line in output.splitlines() if "=" in line)
    if not ok or fields.get("LoadState") != "loaded":
        return "Service unavailable", "Connection unknown", "Unread unavailable", False, False
    state = fields.get("ActiveState")
    if state != "active":
        title = {"inactive": "Stopped", "failed": "Service failed", "activating": "Starting",
                 "deactivating": "Stopping"}.get(state, "Service state unknown")
        return title, "Connection offline" if state in ("inactive", "failed") else "Connection unknown", \
            "Unread unavailable while offline", state in ("inactive", "failed"), state == "activating"
    data = source(fields.get("InvocationID", ""))
    if data is None:
        return "Service running", "Connection unknown · no current status", "Unread unavailable", False, True
    connection = data["connection"]
    count = data.get("unreadMessages")
    metric = {"messenger": "Messenger", "quiver": "Quiver relayed"}.get(data.get("unreadSource"))
    if connection == "online" and type(count) is int and 0 <= count <= 2**64 - 1 and metric:
        unread = (f"Quiver relayed unread (current chats): {count}" if data.get("unreadSource") == "quiver"
                  else f"Messenger unread messages: {count}")
    else:
        reason = data.get("unreadReason")
        unread = "Unread unavailable" + (f" · {reason[:240]}" if isinstance(reason, str) else "")
    return "Service running", f"Connection {connection}", unread, False, True


class Messenger(Panel):
    icon, title, refresh_ms = "\uf075", "Messenger outpost", 5000

    def __init__(self, app):
        super().__init__(app)
        self.busy = False
        self.action_error = ""
        self.unread_target = None
        self.unread_value = None
        self.unread_at = 0
        self.unread_next = 0
        self.unread_busy = False
        self.latest = None
        self.start = button("Start", lambda: self.control("start"))
        self.stop = button("Stop", lambda: self.control("stop"))
        self.start.set_sensitive(False)
        self.stop.set_sensitive(False)
        self.extra.append(self.start)
        self.extra.append(self.stop)

    def control(self, action):
        # Start/stop only: never changes the user's enable-at-login preference.
        if self.busy:
            return
        self.busy = True
        self.start.set_sensitive(False)
        self.stop.set_sensitive(False)

        def done(ok, _output):
            self.busy = False
            self.action_error = "" if ok else f"Could not {action} service. See the service log."
            self.refresh()

        run_async(["systemctl", "--user", action, UNIT], done)

    def build_foot(self):
        self.spacer()
        self.foot_button("Log…", ["dwm-sensible-terminal", "-e", "journalctl", "--user",
                                  "-u", UNIT, "-n", "100", "--no-pager"])

    def refresh(self):
        if self.busy:
            return
        self.busy = True

        def done(ok, output):
            self.busy = False
            fields = dict(line.split("=", 1) for line in output.splitlines() if "=" in line)
            data = (snapshot(fields.get("InvocationID", "")) if ok
                    and fields.get("LoadState") == "loaded" and fields.get("ActiveState") == "active" else None)
            native_count = data and type(data.get("unreadMessages")) is int and 0 <= data["unreadMessages"] <= 2**64 - 1 \
                and data.get("unreadSource") in ("messenger", "quiver")
            target = query_target(data) if data and not native_count else None
            if self.unread_target != target:
                self.unread_target, self.unread_value, self.unread_next = target, None, 0
            self.latest = ok, output, data
            self.draw()
            if target and not self.unread_busy and time.monotonic() >= self.unread_next:
                self.unread_busy = True
                self.unread_next = time.monotonic() + 15

                def counted(success, result):
                    self.unread_busy = False
                    if self.unread_target == target:
                        self.unread_value = query_result(success, result, target)
                        self.unread_at = time.monotonic()
                        self.draw()

                run_async(query_command(target), counted)

        run_async(SHOW, done)

    def draw(self):
        ok, output, data = self.latest
        if data and self.unread_target and self.unread_value and time.monotonic() - self.unread_at <= 20:
            count, reason = self.unread_value
            data = dict(data, unreadSource="quiver", unreadMessages=count, unreadReason=reason)
        state, connection, unread, can_start, can_stop = describe(ok, output, lambda _: data)
        self.sub.set_text(state)
        self.start.set_sensitive(can_start)
        self.stop.set_sensitive(can_stop)
        clear(self.body)
        self.body.append(label(connection, "sub"))
        detail = label(unread, "sub")
        detail.set_wrap(True)
        detail.set_max_width_chars(42)
        self.body.append(detail)
        if self.action_error:
            self.body.append(label(self.action_error, "sub"))
