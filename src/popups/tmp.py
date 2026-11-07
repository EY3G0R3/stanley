"""What fills /tmp, by status, and cleaning out what is stale."""

import json
import os
import re

from gi.repository import Gtk

from common import (
    C, Panel, button, clear, confirm, draw_dot, draw_ring, hbox, human, label, launch, pill, rgb,
    rounded, row, run_async, vbox,
)


def task_name(folder):
    """The task a Claude temp folder belongs to: the worktree name when the
    session runs in one, else its working directory."""
    if "worktrees-" in folder:
        return folder.split("worktrees-", 1)[1]
    home = os.path.expanduser("~").replace("/", "-")
    if folder.startswith(home):
        return "~" + folder[len(home):].replace("-", "/")
    return folder.replace("-", "/")


class Tmp(Panel):
    icon, title = "\U000f0aba", "/tmp"
    COLORS = {"in use": C["cyan"], "stale": C["warn"], "recent": C["accent"],
              "keep": C["ok"], "marked": C["pink"]}

    # Claude Code keeps each session's temp files in CLAUDE_DIR/<cwd>/<session id>,
    # where <cwd> is the session's working directory with "/" turned into "-".
    CLAUDE_DIR = f"/tmp/claude-{os.getuid()}"
    AGENT_MIN = 100 << 20
    ASK = ("From the user, via the stanley /tmp popup: /tmp is short of space. "
           "Your temp folder {path} holds {size}. Delete the build copies and scratch "
           "files you no longer need, keep only what your current work uses, and reuse "
           "one build folder from now on. No reply needed.")

    def __init__(self, app):
        super().__init__(app)
        self.report = None
        self.agents = []
        self.asked = {}
        self.clean_btn = None

    def build_foot(self):
        self.foot_button("Open /tmp", ["xdg-open", "/tmp"])
        self.spacer()
        self.clean_btn = Gtk.Button(label="Clean stale")
        self.clean_btn.add_css_class("btn")
        self.clean_btn.set_sensitive(False)
        confirm(self.clean_btn, self.clean)
        self.foot.append(self.clean_btn)

    def refresh(self):
        if self.report is None:
            clear(self.body)
            self.body.append(label("Scanning /tmp…", "sub"))
        run_async(["tmp-usage", "--json", "--limit", "5"], self.loaded)
        run_async(["du", "-xb", "-d", "2", self.CLAUDE_DIR], self.agents_loaded)

    def agents_loaded(self, _ok, out):
        """One entry per Claude session folder, largest first. du also reports
        unreadable paths, which carry no tab and are skipped."""
        agents = []
        for line in out.splitlines():
            size, _, path = line.partition("\t")
            rel = os.path.relpath(path, self.CLAUDE_DIR) if path else ""
            parts = rel.split(os.sep)
            if len(parts) == 2 and size.isdigit() and int(size) >= self.AGENT_MIN \
                    and re.fullmatch(r"[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}", parts[1]):
                agents.append(dict(name=task_name(parts[0]), session=parts[1],
                                   path=path, size=int(size)))
        self.agents = sorted(agents, key=lambda a: -a["size"])[:5]
        if self.report is not None:
            self.draw()

    def ask(self, agent):
        session = agent["session"]
        self.asked[session] = "Asking…"
        self.draw()
        text = self.ASK.format(path=agent["path"], size=human(agent["size"]))

        def done(ok, out):
            self.asked[session] = "Asked" if '"accepted"' in out or '"injected"' in out \
                else "Not running" if '"undelivered"' in out else "Failed"
            self.draw()
        run_async(["agent-msg", "send", "claude:" + session, "--kind", "request",
                   "--message", text], done)

    def loaded(self, ok, out):
        try:
            self.report = json.loads(out)
        except ValueError:
            clear(self.body)
            self.body.append(label("tmp-usage failed: " + (out.strip()[:80] or "no output"), "sub"))
            return
        self.draw()

    def draw(self):
        r = self.report
        clear(self.body)
        quota = r.get("quota") or {}
        cap = r.get("capacity") or {}
        limited = r.get("limitedBy") == "quota" and quota.get("limit")
        total = quota["limit"] if limited else cap.get("total", 0)
        used = quota["used"] if limited else cap.get("used", 0)
        free = r.get("free", total - used)
        frac = used / total if total else 0
        self.sub.set_text("Limited by your per-user quota" if limited else "Whole filesystem")
        color = C["bad"] if frac >= .9 else C["warn"] if frac >= .75 else C["ok"]

        ring = Gtk.DrawingArea(content_width=92, content_height=92)
        ring.set_draw_func(lambda _a, cr, w, h: draw_ring(cr, w, h, frac, color))
        text = vbox(2)
        text.set_valign(Gtk.Align.CENTER)
        text.append(label(human(free), "big"))
        text.append(label(f"free of {human(total)} · {human(used)} used", "sub"))
        self.body.append(hbox(ring, text, spacing=16))

        buckets = [b for b in r.get("buckets", []) if b.get("size")]
        if buckets:
            self.body.append(label("WHAT USES IT", "sect"))
            bar = Gtk.DrawingArea(content_height=10, hexpand=True)
            bar.set_draw_func(lambda _a, cr, w, h: self.draw_stack(cr, w, h, buckets, max(used, 1)))
            self.body.append(bar)
            legend = vbox(2)
            for b in buckets:
                dot = Gtk.DrawingArea(content_width=9, content_height=9, valign=Gtk.Align.CENTER)
                col = self.COLORS.get(b["status"], C["muted"])
                dot.set_draw_func(lambda _a, cr, w, h, c=col: draw_dot(cr, w, h, c))
                n = b.get("count", 0)
                legend.append(hbox(dot, label(f"{b['label']} · {n} item{'s' if n != 1 else ''}", expand=True),
                                   label(human(b["size"]), "meta"), spacing=8))
            self.body.append(legend)

        largest = r.get("largest", [])[:5]
        if largest:
            box = vbox(4)
            for e in largest:
                kind = {"stale": "warn", "in use": ""}.get(e.get("status"), "")
                name = e["name"] + ("/" if e.get("isDir") else "")
                box.append(row(None, name, hbox(pill(e.get("status", ""), kind), label(human(e["size"]), "meta"), spacing=6),
                               on_click=lambda n=e["name"]: (launch(["xdg-open", os.path.join("/tmp", n)]), self.app.quit())))
            self.body.append(box)

        if self.agents:
            self.body.append(label("AGENTS", "sect"))
            box = vbox(4)
            for a in self.agents:
                state = self.asked.get(a["session"])
                if state:
                    action = pill(state, "ok" if state == "Asked" else "warn")
                else:
                    # An icon keeps the row to one line; the tooltip says what it does.
                    action = button("\U000f00e2", lambda a=a: self.ask(a), css="ic")
                    action.set_tooltip_text("Ask this agent to clean its temp files")
                action.set_valign(Gtk.Align.CENTER)
                box.append(row(None, a["name"], hbox(label(human(a["size"]), "meta"), action, spacing=6)))
            self.body.append(box)

        stale = (r.get("staleCleanup") or {}).get("size", 0)
        if self.clean_btn and not self.clean_btn.has_css_class("armed"):
            self.clean_btn.set_label(f"Clean stale ({human(stale)})" if stale else "Nothing stale")
            self.clean_btn.set_sensitive(bool(stale))

    def clean(self):
        self.clean_btn.remove_css_class("armed")
        self.clean_btn.set_label("Cleaning…")
        self.clean_btn.set_sensitive(False)
        run_async(["tmp-usage", "--cleanup-stale"], lambda *_: self.refresh())

    def draw_stack(self, cr, w, h, buckets, total):
        rgb(cr, C["line"])
        rounded(cr, 0, 0, w, h, h / 2)
        cr.fill()
        cr.save()
        rounded(cr, 0, 0, w, h, h / 2)
        cr.clip()
        x = 0
        for b in buckets:
            bw = max(2, w * b["size"] / total)
            rgb(cr, self.COLORS.get(b["status"], C["muted"]))
            cr.rectangle(x, 0, bw, h)
            cr.fill()
            x += bw
        cr.restore()
