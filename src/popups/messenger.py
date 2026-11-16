"""The Messenger outpost: service controls and an expiring status snapshot."""
import json
import os
import time

from common import Panel, button, clear, label, run_async

UNIT = "quiver-outpost-messenger.service"
SHOW = ["systemctl", "--user", "show", "--property=LoadState,ActiveState,InvocationID", UNIT]


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
    if connection == "online" and type(count) is int and count >= 0 and metric:
        unread = f"{metric} unread messages: {count}"
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
            state, connection, unread, can_start, can_stop = describe(ok, output, snapshot)
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

        run_async(SHOW, done)
