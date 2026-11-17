"""Isolated popup fixtures: no display, user bus, credentials or live commands."""
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest.mock import patch


class Widget:
    def __init__(self, text="", callback=None, child=None, **_):
        self.text, self.callback, self.sensitive = text, callback, True
        self.children, self.css, self.handlers = [child] if child else [], set(), {}

    def __getattr__(self, name):
        if name.startswith(("set_", "grab_")):
            return lambda *_: None
        raise AttributeError(name)

    def add_css_class(self, css):
        self.css.add(css)

    def connect(self, signal, handler):
        self.handlers[signal] = handler

    def get_text(self):
        return self.text

    def append(self, child):
        self.children.append(child)

    def set_sensitive(self, sensitive):
        self.sensitive = sensitive

    def set_text(self, text):
        self.text = text

    def set_wrap(self, wrap):
        self.wrap = wrap

    def set_max_width_chars(self, width):
        self.width = width


class Panel:
    def __init__(self, app):
        self.app = app
        self.sub, self.extra, self.body, self.foot = Widget(), Widget(), Widget(), Widget()

    def spacer(self):
        self.foot.append(Widget())

    def foot_button(self, text, cmd):
        self.foot.append(Widget(text))


def box(*children, **_):
    b = Widget()
    b.children = [c for c in children if isinstance(c, Widget)]
    return b


fake_common = types.SimpleNamespace(
    Panel=Panel, button=lambda text, cb, *_: Widget(text, cb),
    clear=lambda box: box.children.clear(), label=lambda text, *_, **__: Widget(text),
    run_async=lambda *_: None, hbox=box, vbox=box, launch=lambda *_: None,
    GLib=types.SimpleNamespace(idle_add=lambda f: f()),
    Gtk=types.SimpleNamespace(Button=Widget, Entry=Widget),
)
spec = importlib.util.spec_from_file_location(
    "messenger_popup", Path(__file__).resolve().parents[1] / "src/popups/messenger.py")
popup = importlib.util.module_from_spec(spec)
with patch.dict(sys.modules, common=fake_common):
    spec.loader.exec_module(popup)


def service(state="active", load="loaded"):
    return f"LoadState={load}\nActiveState={state}\nInvocationID=abc\n"


class MessengerTests(unittest.TestCase):
    def test_render_fixture_states(self):
        cases = [(True, "inactive", None, "Stopped", "Unread unavailable while offline", True, False),
                 (True, "active", None, "Service running", "Unread unavailable", False, True),
                 (False, "active", None, "Service unavailable", "Unread unavailable", False, False),
                 (True, "failed", None, "Service failed", "Unread unavailable while offline", True, False)]
        for count in (0, 8):
            cases.append((True, "active", {"connection": "online", "unreadSource": "messenger",
                                           "unreadMessages": count}, "Service running",
                          f"Messenger unread messages: {count}", False, True))
        for ok, state, snapshot, title, unread, start, stop in cases:
            with self.subTest(state=state, snapshot=snapshot):
                view = popup.Messenger(None)
                with patch.object(popup, "snapshot", return_value=snapshot), \
                        patch.object(popup, "run_async", side_effect=lambda cmd, cb: cb(ok, service(state))):
                    view.refresh()
                self.assertEqual(view.sub.text, title)
                self.assertEqual(view.status.children[1].text, unread)
                self.assertEqual((view.start.sensitive, view.stop.sensitive), (start, stop))

    def test_actions_only_start_stop_and_show_errors(self):
        view = popup.Messenger(None)
        commands = []

        def run(cmd, callback):
            commands.append(cmd)
            callback(cmd == popup.SHOW, service("inactive"))

        with patch.object(popup, "run_async", side_effect=run):
            view.start.callback()
            self.assertEqual(commands[0], ["systemctl", "--user", "start", popup.UNIT])
            self.assertIn("Could not start", view.status.children[-1].text)
            commands.clear()
            view.stop.callback()
            self.assertEqual(commands[0], ["systemctl", "--user", "stop", popup.UNIT])
        self.assertFalse(any("enable" in cmd or "disable" in cmd for cmd in commands))

    def test_source_validation_and_semantics(self):
        with tempfile.TemporaryDirectory() as runtime, patch.dict(os.environ, XDG_RUNTIME_DIR=runtime):
            directory = Path(runtime) / "quiver-outpost-messenger"
            directory.mkdir()
            path = directory / "status.json"
            data = dict(version=1, updatedAt=100, invocationId="abc", connection="online",
                        unreadMessages=0, unreadSource="quiver")
            path.write_text(json.dumps(data))
            self.assertEqual(popup.snapshot("abc", now=100), data)
            self.assertIsNone(popup.snapshot("other", now=100))
            self.assertIsNone(popup.snapshot("abc", now=131))
            self.assertIsNone(popup.snapshot("abc", now=99))
            self.assertIn("Quiver relayed unread messages: 0", popup.describe(True, service(), lambda _: data))
            for count in (None, -1, 1.5, True, "0"):
                data["unreadMessages"] = count
                self.assertIn("Unread unavailable", popup.describe(True, service(), lambda _: data)[2])
            data["unreadMessages"], data["connection"] = 8, "offline"
            self.assertIn("Unread unavailable", popup.describe(True, service(), lambda _: data)[2])
            path.write_text("not json")
            self.assertIsNone(popup.snapshot("abc", now=100))
            path.write_text("[]")
            self.assertIsNone(popup.snapshot("abc", now=100))

    def test_unread_chats_label_reason_and_offline(self):
        def unread(**fields):
            data = dict(connection="online", unreadSource="messenger", unreadMessages=None)
            data.update(fields)
            return popup.describe(True, service(), lambda _: data)[2]

        self.assertEqual(unread(unreadChats=0), "Messenger unread chats: 0")
        self.assertEqual(unread(unreadChats=3), "Messenger unread chats: 3")
        self.assertEqual(unread(unreadMessages=5, unreadChats=3), "Messenger unread messages: 5")
        for count in (-1, 1.5, True, "3"):
            self.assertEqual(unread(unreadChats=count), "Unread unavailable")
        self.assertEqual(unread(unreadChats=None, unreadChatsReason="Read markers missing",
                                unreadReason="old"), "Unread unavailable · Read markers missing")
        self.assertEqual(unread(unreadChats=None, unreadReason="old"), "Unread unavailable · old")
        self.assertEqual(unread(connection="offline", unreadChats=3), "Unread unavailable")
        self.assertNotIn("messages", unread(unreadChats=3))


def texts(widget):
    """Every text under a fake widget, depth first."""
    found = [widget.text] if widget.text else []
    for child in widget.children:
        found += texts(child)
    return found


class Thread:
    def __init__(self, target, daemon):
        self.target = target

    def start(self):
        self.target()


class Proc:
    def __init__(self, code=0, out="", err=""):
        self.returncode, self.stdout, self.stderr = code, out, err


CHATS = [{"threadId": f"t{i}", "name": f"Chat {i}", "group": False,
          "lastActivityAt": (1000 - 90 * i) * 1000, "lastText": f"hi {i}",
          "breakoutChannelId": None} for i in range(5)]


class ChatTests(unittest.TestCase):
    def setUp(self):
        self.calls, self.replies = [], []
        patches = [patch.object(popup.threading, "Thread", Thread),
                   patch.object(popup.subprocess, "run", side_effect=self.run_fake),
                   patch.object(popup.time, "time", return_value=1000)]
        for p in patches:
            p.start()
            self.addCleanup(p.stop)
        self.view = popup.Messenger(types.SimpleNamespace(quit=lambda: self.calls.append("quit")))

    def run_fake(self, argv, cwd, input, capture_output, text, timeout):
        self.calls.append((argv, input))
        self.assertEqual((cwd, timeout), (popup.QUIVER_DIR, 10))
        reply = self.replies.pop(0)
        if isinstance(reply, Exception):
            raise reply
        return reply

    def load(self, reply):
        self.replies.append(reply)
        self.view.load_chats()
        return texts(self.view.chats)

    def test_chats_listed_truncated_with_exact_argv(self):
        rows = [dict(r) for r in CHATS] + [dict(CHATS[0], threadId="t5")]
        rows[1]["lastText"] = "line one\nline two " + "x" * 80
        shown = self.load(Proc(out=json.dumps(rows)))
        self.assertEqual(self.calls, [(["quiver", "call", "-c", "@quiver-messenger",
                                        "quiver-messenger.chats", '{"limit": 5}'], None)])
        self.assertEqual(len(self.view.rows), 5)
        for name in ("Chat 0", "Chat 4", "hi 0", "now", "1m", "6m"):
            self.assertIn(name, shown)
        preview = next(t for t in shown if t.startswith("line one"))
        self.assertEqual(len(preview), 40)
        self.assertTrue(preview.endswith("…"))
        self.assertNotIn("\n", preview)

    def test_empty_failure_and_timeout(self):
        self.assertIn("No chats yet", self.load(Proc(out="[]")))
        shown = self.load(Proc(code=1, err="Error: Only the member whose outpost runs here"))
        self.assertIn("Chats unavailable · Error: Only the member whose outpost runs here", shown)
        self.assertIn("Chats unavailable · timed out",
                      self.load(popup.subprocess.TimeoutExpired("quiver", 10)))
        self.assertIn("Chats unavailable · unreadable reply", self.load(Proc(out="{}")))

    def reply_widgets(self):
        self.load(Proc(out=json.dumps(CHATS)))
        item = self.view.chats.children[1]
        item.handlers["clicked"]()
        reply = self.view.chats.children[2]
        entry, send = reply.children[0].children
        return entry, send

    def test_send_success_uses_stdin_and_exact_argv(self):
        entry, send = self.reply_widgets()
        self.assertEqual(self.view.open_thread, "t0")
        entry.text = "  hello there "
        self.replies.append(Proc(out='{"outboxId":"api-1"}'))
        send.callback()
        argv, stdin = self.calls[-1]
        self.assertEqual(argv, ["quiver", "call", "-c", "@quiver-messenger",
                                "quiver-messenger.sendMessage", "-"])
        self.assertEqual(json.loads(stdin), {"threadId": "t0", "text": "hello there"})
        self.assertEqual(entry.text, "")
        self.assertIn("Sent", texts(self.view.chats))

    def test_send_failure_keeps_text_and_empty_is_ignored(self):
        entry, send = self.reply_widgets()
        entry.handlers["activate"]()
        self.assertEqual(len(self.calls), 1)  # empty entry sends nothing
        entry.text = "hello"
        self.replies.append(Proc(code=1, err="Error: The message is too long."))
        entry.handlers["activate"]()
        self.assertEqual(entry.text, "hello")
        self.assertTrue(send.sensitive)
        self.assertIn("Not sent · Error: The message is too long.", texts(self.view.chats))

    def test_open_quiver_and_open_chat_argv(self):
        opened = []
        with tempfile.TemporaryDirectory() as root, \
                patch.object(popup, "QUIVER_DIR", root), \
                patch.object(popup, "launch", side_effect=opened.append):
            os.mkdir(os.path.join(root, ".quiver"))
            with open(os.path.join(root, ".quiver", "config.json"), "w") as f:
                json.dump({"baseUrl": "https://q.example/"}, f)
            self.view.foot.children.clear()
            self.view.build_foot()
            self.view.foot.children[0].callback()
            self.assertEqual(opened, [["xdg-open", "https://q.example/~/c/@quiver-messenger"]])
            self.replies.append(Proc(out='{"breakoutChannelId":"ch123"}'))
            self.calls.clear()
            self.view.open_chat("t1")
            self.assertEqual(self.calls[0], (["quiver", "call", "-c", "@quiver-messenger",
                                              "quiver-messenger.openChat", '{"threadId": "t1"}'], None))
            self.assertEqual(opened[-1], ["xdg-open", "https://q.example/~/c/+ch123"])
            self.assertEqual(self.calls[-1], "quit")
            os.remove(os.path.join(root, ".quiver", "config.json"))
            self.view.open_quiver()
            self.assertEqual(len(opened), 2)
            self.assertIn("Quiver address unknown", texts(self.view.chats))

    def test_send_runs_only_on_explicit_action(self):
        self.reply_widgets()
        self.view.toggle("t0")  # collapsing, like closing, sends nothing
        self.assertEqual(len(self.calls), 1)


if __name__ == "__main__":
    unittest.main()
