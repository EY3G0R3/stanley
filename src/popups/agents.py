"""Claude Code and Codex quota."""

import os
import time
from datetime import datetime

from gi.repository import Gtk

from common import C, Panel, button, clear, draw_bar, hbox, label, launch, run_async, vbox


class Agents(Panel):
    """Weekly quota per agent, from the files ~/bin/agent-quota writes."""
    icon, title = "󰚩", "Agents"
    AGENTS = (("claude", "Claude Code"), ("codex", "Codex"))
    DIR = os.path.join(os.environ.get("XDG_STATE_HOME") or os.path.expanduser("~/.local/state"), "agent-quota")

    WEEK = 7 * 86400

    COMMON = (
        "From the user, via the stanley Agents popup: where did my {name} tokens go? "
        "Look at usage since {since}, the start of the current weekly quota window "
        "(now at {percent}%). {data} Break it down per Dwight task (dwight task list, "
        "then dwight task usage -t ID --since ...) and per session, including "
        "sessions that belong to no task. Show the top consumers in a short ranked "
        "table with tokens, share of the total, model, and what the work was. Then "
        "point out what drove the cost and give concrete tips to spend less. "
        "Read only; change nothing.")
    DATA = {
        "claude": "Claude Code transcripts are ~/.claude/projects/<cwd>/<session>.jsonl; "
                  "each assistant message carries usage (input, output, cache_read and "
                  "cache_creation input tokens) and the model. Count subagent sidechains "
                  "too, and watch for cache misses, long contexts, subagent fan-out, big "
                  "tool outputs, and Opus where a smaller model would do.",
        "codex": "Codex rollouts are ~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl; "
                 "token_count events carry cumulative total_token_usage (input, "
                 "cached_input, output, reasoning_output), so take the last one per "
                 "session. Watch for low cache hit rates, heavy reasoning effort, long "
                 "contexts, and review or retry loops.",
    }

    def __init__(self, app):
        super().__init__(app)
        self.fetched = False

    def investigate(self, agent, name, percent, reset):
        """Open a tab with a fresh agent of the same kind, asked where its tokens went."""
        start = reset - self.WEEK if reset > time.time() else time.time() - self.WEEK
        since = datetime.fromtimestamp(start).astimezone().isoformat(timespec="seconds")
        prompt = self.COMMON.format(name=name, since=since, percent=percent, data=self.DATA[agent])
        # Without a socket, Dwight opens the tab in the main Sprinkles window
        # (unix:@dwight-main), starting it if needed. Drop any inherited kitty
        # target so the tab cannot land elsewhere.
        launch(["env", "-u", "KITTY_LISTEN_ON", "-u", "KITTY_WINDOW_ID",
                "dwight", "project", "open", os.path.expanduser("~"), "--new",
                "--agent", agent, "--prompt", prompt])
        self.app.quit()

    @staticmethod
    def bar(name, frac, color):
        """A thin bar with a short label, so the tokens and time bars line up."""
        area = Gtk.DrawingArea(content_height=10, hexpand=True, valign=Gtk.Align.CENTER)
        area.set_draw_func(lambda _a, cr, w, h: draw_bar(cr, w, h, frac, color))
        name = label(name, "sub")
        # Wide enough for the longest label, so both bars start and end together.
        name.set_width_chars(len("tokens"))
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
            over = False
            if left <= 0:
                percent, week, when = 0, 0, "reset, waiting for new numbers"
            else:
                week = min(1, max(0, 1 - left / self.WEEK))
                d, h, m = int(left // 86400), int(left % 86400 // 3600), int(left % 3600 // 60)
                # Linear forecast at the current rate, once a day has passed.
                start = reset - self.WEEK
                if time.time() - start < 86400:
                    pace = "too early to tell"
                elif (forecast := percent / week) <= 100:
                    pace = f"at this rate: {round(forecast)}% by reset"
                else:
                    over = True
                    out = start + self.WEEK * week * 100 / percent
                    pace = time.strftime("at this rate: out %a %-I:%M %p", time.localtime(out))
                when = (pace + " · " + time.strftime("resets %a %-I:%M %p", time.localtime(reset))
                        + " · in " + (f"{d}d {h}h" if d else f"{h}h {m}m"))
            frac = percent / 100
            color = C["bad"] if frac >= .9 else C["warn"] if over else C["ok"]
            box = vbox(6)
            # A small magnifier: a text button drew more attention than the number.
            investigate = button("\U000f0349", lambda a=agent, n=name, p=percent, r=reset:
                                 self.investigate(a, n, p, r), css="ic")
            investigate.set_valign(Gtk.Align.CENTER)
            box.append(hbox(label(name, "name", expand=True), investigate, label(f"{percent}%", "big")))
            box.append(self.bar("tokens", frac, color))
            box.append(self.bar("time", week, C["muted"]))
            box.append(label(when, "sub"))
            self.body.append(box)
        if self.body.get_first_child() is None:
            self.body.append(label("No quota yet: is agent-quota.timer running?", "sub"))
        # The files can be up to five minutes old; fetch once on open.
        if not self.fetched:
            self.fetched = True
            run_async(["agent-quota"], lambda *_: self.refresh())
