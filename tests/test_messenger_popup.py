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
    def __init__(self, text="", callback=None):
        self.text, self.callback, self.sensitive = text, callback, True
        self.children = []

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
        self.sub, self.extra, self.body = Widget(), Widget(), Widget()


fake_common = types.SimpleNamespace(
    Panel=Panel, button=lambda text, cb: Widget(text, cb),
    clear=lambda box: box.children.clear(), label=lambda text, *_: Widget(text),
    run_async=lambda *_: None,
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
                self.assertEqual(view.body.children[1].text, unread)
                self.assertTrue(view.body.children[1].wrap)
                self.assertEqual(view.body.children[1].width, 42)
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
            self.assertIn("Could not start", view.body.children[-1].text)
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
            self.assertIn("Quiver relayed unread (current chats): 0", popup.describe(True, service(), lambda _: data))
            for count in (None, -1, 1.5, True, "0"):
                data["unreadMessages"] = count
                self.assertIn("Unread unavailable", popup.describe(True, service(), lambda _: data)[2])
            data["unreadMessages"], data["connection"] = 8, "offline"
            self.assertIn("Unread unavailable", popup.describe(True, service(), lambda _: data)[2])
            path.write_text("not json")
            self.assertIsNone(popup.snapshot("abc", now=100))
            path.write_text("[]")
            self.assertIsNone(popup.snapshot("abc", now=100))

    def test_read_only_query_is_bound_to_owner_and_installation(self):
        data = dict(connection="online", unreadMessages=None, unreadSource="messenger",
                    quiverUrl="https://example.test", channelId="abc123", appId="quiver-messenger",
                    ownerHandle="owner", ownerMemberId="human123", invocationId="abc")
        target = popup.query_target(data)
        self.assertEqual(popup.query_command(target), ["timeout", "--kill-after=1s", "8s", "quiver",
                         "--as", "owner@example.test", "messages", "app-unread", "--channel-id", "abc123",
                         "--app", "quiver-messenger", "--key-prefix", "in:", "--member-id", "human123", "--json"])
        for count in (0, 8):
            self.assertEqual(popup.query_result(True, json.dumps(dict(memberHandle="owner", unreadMessages=count)), target)[0], count)
        for value in (dict(memberHandle="other", unreadMessages=8),
                      dict(memberHandle="owner", unreadMessages=None, unreadReason="scan incomplete"),
                      dict(memberHandle="owner", unreadMessages=True)):
            self.assertIsNone(popup.query_result(True, json.dumps(value), target)[0])
        for origin in ("https://user:secret@example.test", "https://example.test/path", "http://example.test", "https://127.0.0.1:8080"):
            self.assertIsNone(popup.query_target(dict(data, quiverUrl=origin)))
        self.assertIsNotNone(popup.query_target(dict(data, quiverUrl="http://127.0.0.1:8080")))
        self.assertIsNone(popup.query_target(dict(data, connection="offline")))

    def test_query_renders_zero_and_discards_result_when_offline(self):
        view = popup.Messenger(None)
        callbacks = []
        data = dict(connection="online", unreadMessages=None, unreadSource="messenger",
                    quiverUrl="https://example.test", channelId="abc123", appId="quiver-messenger",
                    ownerHandle="owner", ownerMemberId="human123", invocationId="abc")
        with patch.object(popup, "snapshot", return_value=data), \
                patch.object(popup, "run_async", side_effect=lambda cmd, cb: callbacks.append((cmd, cb))):
            view.refresh()
            callbacks.pop(0)[1](True, service())
            query = callbacks.pop(0)
            query[1](True, '{"memberHandle":"owner","unreadMessages":0}')
            self.assertEqual(view.body.children[1].text, "Quiver relayed unread (current chats): 0")
            view.refresh()
            callbacks.pop(0)[1](True, service("inactive"))
            query[1](True, '{"memberHandle":"owner","unreadMessages":8}')
            self.assertEqual(view.sub.text, "Stopped")
            self.assertEqual(view.body.children[1].text, "Unread unavailable while offline")


if __name__ == "__main__":
    unittest.main()
