"""The Messenger outpost: service controls, an expiring status snapshot, and
recent chats read and answered through the quiver-messenger app."""
import json
import os
import subprocess
import threading
import time

from common import GLib, Gtk, Panel, button, clear, hbox, label, launch, run_async, vbox

UNIT = "quiver-outpost-messenger.service"
SHOW = ["systemctl", "--user", "show", "--property=LoadState,ActiveState,InvocationID", UNIT]

# The service runs quiver from this workspace, which resolves its identity.
QUIVER_DIR = os.path.expanduser("~/src/morkovka.quiver.city")
APP = "quiver-messenger"
CALL = ["quiver", "call", "-c", "@quiver-messenger"]
CHAT_LIMIT = 5
TIMEOUT = 10


def quiver(args, done, stdin=None):
    """Call the app off the GTK thread; done(ok, output) runs on it.

    The platform fills in `caller`, so arguments omit it. Output and message
    text stay in memory: nothing here writes them to disk or a log."""
    def work():
        try:
            proc = subprocess.run(CALL + args, cwd=QUIVER_DIR, input=stdin, capture_output=True,
                                  text=True, timeout=TIMEOUT)
            ok = proc.returncode == 0
            out = proc.stdout if ok else (proc.stderr.strip() or proc.stdout.strip() or "failed")
        except subprocess.TimeoutExpired:
            ok, out = False, "timed out"
        except OSError as err:
            ok, out = False, err.strerror or "quiver not found"
        GLib.idle_add(lambda: (done(ok, out), False)[1])

    threading.Thread(target=work, daemon=True).start()


def base_url():
    try:
        with open(os.path.join(QUIVER_DIR, ".quiver", "config.json")) as f:
            url = json.loads(f.read(8192))["baseUrl"]
        return url.rstrip("/") if isinstance(url, str) and url.startswith("https://") else None
    except (OSError, KeyError, ValueError, TypeError):
        return None


def short(text, width=40):
    """One line, cut to width."""
    line = " ".join(str(text or "").split())
    return line if len(line) <= width else line[:width - 1] + "…"


def ago(ms, now=None):
    if type(ms) is not int or ms <= 0:
        return ""
    sec = max(0, (time.time() if now is None else now) - ms / 1000)
    for unit, size in (("d", 86400), ("h", 3600), ("m", 60)):
        if sec >= size:
            return f"{int(sec // size)}{unit}"
    return "now"


def parse_chats(output):
    rows = json.loads(output)
    if not isinstance(rows, list):
        raise ValueError
    return [r for r in rows if isinstance(r, dict) and isinstance(r.get("threadId"), str)][:CHAT_LIMIT]


def first_line(text):
    return short(text.strip().splitlines()[-1] if text.strip() else "failed", 80)


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
    chats = data.get("unreadChats")
    if connection == "online" and type(count) is int and count >= 0 and metric:
        unread = f"{metric} unread messages: {count}"
    elif connection == "online" and type(chats) is int and chats >= 0:
        # A chat count is never a message count; keep the label explicit.
        unread = f"Messenger unread chats: {chats}"
    else:
        reason = data.get("unreadChatsReason")
        if not isinstance(reason, str):
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
        self.status = vbox(12)
        self.chats = vbox(2)
        self.body.append(self.status)
        self.body.append(self.chats)
        self.rows = None  # None until loaded; message text lives only here
        self.chat_error = ""
        self.open_error = ""
        self.open_thread = None
        self.notes = {}  # threadId -> "Sent" or an error
        self.sending = False

    def widget(self):
        self.load_chats()
        return super().widget()

    # ---------------------------------------------------------------- chats

    def load_chats(self):
        clear(self.chats)
        self.chats.append(label("RECENT CHATS", "sect"))
        self.chats.append(label("Loading chats…", "sub"))

        def done(ok, output):
            try:
                self.rows = parse_chats(output) if ok else None
                self.chat_error = "" if ok else f"Chats unavailable · {first_line(output)}"
            except (ValueError, TypeError):
                self.rows, self.chat_error = None, "Chats unavailable · unreadable reply"
            self.render_chats()

        quiver([f"{APP}.chats", json.dumps({"limit": CHAT_LIMIT})], done)

    def render_chats(self):
        clear(self.chats)
        self.chats.append(label("RECENT CHATS", "sect"))
        if self.open_error:
            self.chats.append(label(self.open_error, "sub"))
        if self.rows is None:
            self.chats.append(label(self.chat_error or "Chats unavailable", "sub"))
            return
        if not self.rows:
            self.chats.append(label("No chats yet", "sub"))
            return
        for chat in self.rows:
            thread = chat["threadId"]
            head = hbox(label(short(chat.get("name") or "Unnamed chat", 28), expand=True, ellipsize=True),
                        label(ago(chat.get("lastActivityAt")), "meta"))
            content = vbox(2)
            content.append(head)
            content.append(label(short(chat.get("lastText")), "sub", expand=True, ellipsize=True))
            item = Gtk.Button(child=content)
            item.add_css_class("row")
            if thread == self.open_thread:
                item.add_css_class("cur")
            item.connect("clicked", lambda *_, t=thread: self.toggle(t))
            self.chats.append(item)
            if thread == self.open_thread:
                self.chats.append(self.reply_box(thread))

    def toggle(self, thread):
        self.open_thread = None if thread == self.open_thread else thread
        self.render_chats()

    def reply_box(self, thread):
        entry = Gtk.Entry(hexpand=True, placeholder_text="Reply")
        send = button("Send", lambda: self.send(thread, entry, send))
        entry.connect("activate", lambda *_: self.send(thread, entry, send))
        box = vbox(4)
        box.append(hbox(entry, send, spacing=8))
        note = self.notes.get(thread)
        box.append(hbox(label(note or "", "sub", expand=True, ellipsize=True),
                        button("Open chat", lambda: self.open_chat(thread), "btn small"), spacing=8))
        GLib.idle_add(lambda: (entry.grab_focus(), False)[1])  # once it is shown
        return box

    def send(self, thread, entry, send):
        # Only an explicit Send or Enter gets here; closing the popup never sends.
        text = entry.get_text().strip()
        if self.sending or not text:
            return
        self.sending = True
        send.set_sensitive(False)

        def done(ok, output):
            self.sending = False
            self.notes[thread] = "Sent" if ok else f"Not sent · {first_line(output)}"
            if ok:
                entry.set_text("")
            send.set_sensitive(True)
            self.render_chats()

        quiver([f"{APP}.sendMessage", "-"], done,
               stdin=json.dumps({"threadId": thread, "text": text}))

    def open_url(self, path):
        base = base_url()
        if not base:
            return False
        launch(["xdg-open", base + path])
        self.app.quit()
        return True

    def open_quiver(self):
        if not self.open_url("/~/c/@quiver-messenger"):
            self.open_error = "Quiver address unknown"
            self.render_chats()

    def open_chat(self, thread):
        def done(ok, output):
            try:
                channel = json.loads(output)["breakoutChannelId"] if ok else None
            except (ValueError, KeyError, TypeError):
                channel = None
            if isinstance(channel, str) and channel and self.open_url(f"/~/c/+{channel}"):
                return
            self.notes[thread] = "Could not open chat" + (f" · {first_line(output)}" if not ok else "")
            self.render_chats()

        quiver([f"{APP}.openChat", json.dumps({"threadId": thread})], done)

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
        self.foot.append(button("Open in Quiver", self.open_quiver))
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
            clear(self.status)
            self.status.append(label(connection, "sub"))
            detail = label(unread, "sub")
            detail.set_wrap(True)
            detail.set_max_width_chars(42)
            self.status.append(detail)
            if self.action_error:
                self.status.append(label(self.action_error, "sub"))

        run_async(SHOW, done)
