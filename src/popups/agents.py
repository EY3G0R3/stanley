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

    WEEK = 7 * 86400

    def __init__(self, app):
        super().__init__(app)
        self.fetched = False

    @staticmethod
    def bar(name, frac, color):
        """A thin bar with a short label, so the used and week bars line up."""
        area = Gtk.DrawingArea(content_height=10, hexpand=True, valign=Gtk.Align.CENTER)
        area.set_draw_func(lambda _a, cr, w, h: draw_bar(cr, w, h, frac, color))
        name = label(name, "sub")
        name.set_size_request(40, -1)
        return hbox(name, area)

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
                percent, week, when = 0, 0, "reset, waiting for new numbers"
            else:
                week = min(1, max(0, 1 - left / self.WEEK))
                d, h, m = int(left // 86400), int(left % 86400 // 3600), int(left % 3600 // 60)
                gap = percent - round(week * 100)
                pace = (f"{gap}% ahead of pace" if gap > 0 else
                        f"{-gap}% under pace" if gap < 0 else "on pace")
                when = (pace + " · " + time.strftime("resets %a %-I:%M %p", time.localtime(reset))
                        + " · in " + (f"{d}d {h}h" if d else f"{h}h {m}m"))
            frac = percent / 100
            color = C["bad"] if frac >= .9 else C["warn"] if frac > week else C["ok"]
            box = vbox(6)
            box.append(hbox(label(name, "name", expand=True), label(f"{percent}%", "big")))
            box.append(self.bar("used", frac, color))
            box.append(self.bar("week", week, C["muted"]))
            box.append(label(when, "sub"))
            self.body.append(box)
        if self.body.get_first_child() is None:
            self.body.append(label("No quota yet: is agent-quota.timer running?", "sub"))
        # The files can be up to five minutes old; fetch once on open.
        if not self.fetched:
            self.fetched = True
            run_async(["agent-quota"], lambda *_: self.refresh())
