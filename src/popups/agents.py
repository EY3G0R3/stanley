"""Claude Code and Codex quota."""

import os
import time

from gi.repository import Gtk

from common import C, Panel, clear, draw_bar, hbox, label, run_async, vbox


class Agents(Panel):
    """Weekly quota per agent, from the files ~/bin/agent-quota writes."""
    icon, title = "󰚩", "Agents"
    AGENTS = (("claude", "Claude Code"), ("codex", "Codex"))
    DIR = os.path.join(os.environ.get("XDG_STATE_HOME") or os.path.expanduser("~/.local/state"), "agent-quota")

    def __init__(self, app):
        super().__init__(app)
        self.fetched = False

    def refresh(self):
        clear(self.body)
        self.sub.set_text("Weekly usage")
        for agent, name in self.AGENTS:
            try:
                percent, reset = (int(x) for x in open(os.path.join(self.DIR, agent)).read().split())
            except (OSError, ValueError):
                continue
            left = reset - time.time()
            if left <= 0:
                percent, when = 0, "reset, waiting for new numbers"
            else:
                d, h, m = int(left // 86400), int(left % 86400 // 3600), int(left % 3600 // 60)
                when = (time.strftime("resets %a %-I:%M %p", time.localtime(reset))
                        + " · in " + (f"{d}d {h}h" if d else f"{h}h {m}m"))
            frac = percent / 100
            color = C["bad"] if frac >= .9 else C["warn"] if frac >= .75 else C["ok"]
            bar = Gtk.DrawingArea(content_height=10, hexpand=True)
            bar.set_draw_func(lambda _a, cr, w, h, f=frac, c=color: draw_bar(cr, w, h, f, c))
            box = vbox(6)
            box.append(hbox(label(name, "name", expand=True), label(f"{percent}%", "big")))
            box.append(bar)
            box.append(label(when, "sub"))
            self.body.append(box)
        if self.body.get_first_child() is None:
            self.body.append(label("No quota yet: is agent-quota.timer running?", "sub"))
        # The files can be up to five minutes old; fetch once on open.
        if not self.fetched:
            self.fetched = True
            run_async(["agent-quota"], lambda *_: self.refresh())
