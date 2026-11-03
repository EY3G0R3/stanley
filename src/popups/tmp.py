"""What fills /tmp, by status, and cleaning out what is stale."""

import json
import os

from gi.repository import Gtk

from common import (
    C, Panel, clear, confirm, draw_dot, draw_ring, hbox, human, label, launch, pill, rgb,
    rounded, row, run_async, vbox,
)


class Tmp(Panel):
    icon, title = "\U000f0aba", "/tmp"
    COLORS = {"in use": C["cyan"], "stale": C["warn"], "recent": C["accent"],
              "keep": C["ok"], "marked": C["pink"]}

    def __init__(self, app):
        super().__init__(app)
        self.report = None
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
