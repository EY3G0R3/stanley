"""The fix-my-messages agent: on or off, its latest fixes and misses."""

import os
import re
import time

from gi.repository import Gtk

from common import Panel, clear, hbox, label, row, run, run_async, vbox


class Messages(Panel):
    """Paddle's fix-my-messages agent (~/bin/fix-my-messages): on or off,
    which proofreader it uses, and what it fixed or missed lately."""
    icon, title, refresh_ms = "\U000f04c6", "Message fixer", 3000
    UNIT = "fix-my-messages"
    CHECKERS = (("claude", "Sonnet · your plan"), ("codex", "your plan"),
                ("paddle", "paid in sparks"))
    STATE = os.path.expanduser("~/.local/state/fix-my-messages")
    FIX = re.compile(r"^(\S+) \S+ \S+: (.*?): (fixed|would fix) (.*)$")
    # A message that reached the agent too late to check in time.
    SKIP = re.compile(r"^(\S+) \S+ \S+: (.*?): (skipped), (arrived after \S+)$")

    def __init__(self, app):
        super().__init__(app)
        self.switch = Gtk.Switch(valign=Gtk.Align.CENTER)
        self.switch_handler = self.switch.connect("state-set", self.set_on)
        self.extra.append(self.switch)

    def build_foot(self):
        self.spacer()
        self.foot_button("Log…", ["dwm-sensible-terminal", "-e", "journalctl", "--user",
                                  "-u", self.UNIT, "-f", "-n", "100"])

    def checker(self):
        try:
            return open(os.path.join(self.STATE, "checker")).read().strip() or "claude"
        except OSError:
            return "claude"

    def watching(self):
        """Whether the service still holds the agent and its message stream:
        the wrapper can outlive them, and then nothing gets checked."""
        group = run(["systemctl", "--user", "show", "-p", "ControlGroup", "--value", self.UNIT]).strip()
        cmds = []
        try:
            for pid in open(f"/sys/fs/cgroup{group}/cgroup.procs").read().split():
                try:
                    cmds.append(open(f"/proc/{pid}/cmdline", "rb").read().replace(b"\0", b" ").decode())
                except OSError:
                    pass
        except OSError:
            return False
        return (any("fix-my-messages.ts" in c for c in cmds)
                and any("messages list --follow" in c for c in cmds))

    def set_on(self, _sw, on):
        run_async(["systemctl", "--user", "enable" if on else "disable", "--now", self.UNIT],
                  lambda *_: self.refresh())
        return False

    def set_checker(self, name):
        os.makedirs(self.STATE, exist_ok=True)
        with open(os.path.join(self.STATE, "checker"), "w") as f:
            f.write(name + "\n")
        # try-restart: a stopped fixer stays stopped and picks it up on start.
        run_async(["systemctl", "--user", "try-restart", self.UNIT], lambda *_: self.refresh())
        self.refresh()

    def refresh(self):
        on = run(["systemctl", "--user", "is-active", self.UNIT]).strip() == "active"
        self.switch.handler_block(self.switch_handler)
        self.switch.set_active(on)
        self.switch.handler_unblock(self.switch_handler)
        current = self.checker()
        fixes = []
        for line in run(["journalctl", "--user", "-u", self.UNIT, "-o", "short-iso",
                         "-n", "1000", "--no-pager"]).splitlines():
            m = self.FIX.match(line) or self.SKIP.match(line)
            if m:
                fixes.append(m.groups())
        today = time.strftime("%Y-%m-%d")
        count = sum(1 for when, _w, kind, _c in fixes if kind == "fixed" and when.startswith(today))
        # Misses: skipped as too slow, or a fix found after the edit window.
        missed = sum(1 for when, _w, kind, changes in fixes if when.startswith(today)
                     and (kind == "skipped" or "(too late" in changes))
        state = ("on" if self.watching() else "on · not watching") if on else "off"
        self.sub.set_text(state + f" · {current}"
                          + (f" · {count} fixed today" if count else "")
                          + (f" · {missed} missed" if missed else ""))

        clear(self.body)
        self.body.append(label("CHECKER", "sect"))
        box = vbox(2)
        for name, about in self.CHECKERS:
            box.append(row(None, name, about, on_click=lambda n=name: self.set_checker(n),
                           current=name == current))
        self.body.append(box)

        self.body.append(label("LATEST FIXES AND MISSES", "sect"))
        if not fixes:
            self.body.append(label("Nothing fixed yet", "sub"))
            return
        box = vbox(6)
        for when, where, kind, changes in reversed(fixes[-6:]):
            head = hbox(label(where, "meta", expand=True, ellipsize=True),
                        label(when[11:16] + {"fixed": "", "skipped": " · skipped"}.get(kind, " · not edited"),
                              "meta"))
            text = Gtk.Label(label=changes, xalign=0, wrap=True, max_width_chars=48)
            entry = vbox(1)
            entry.append(head)
            entry.append(text)
            box.append(entry)
        self.body.append(box)
