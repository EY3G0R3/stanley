"""The base the cpu, memory and disk popups share: a ring for the
headline number, refreshed while the popup is open."""

import os

from gi.repository import Gtk

from common import C, Panel, draw_bar, draw_dot, draw_ring, hbox, label, rgb, rounded


class Gauge(Panel):
    """A live panel: a ring for the headline number, refreshed every
    REFRESH ms, drawn with common's helpers."""
    refresh_ms = 1500

    def ring(self, frac, color):
        ring = Gtk.DrawingArea(content_width=92, content_height=92)
        ring.set_draw_func(lambda _a, cr, w, h: draw_ring(cr, w, h, frac, color))
        return ring

    def bar(self, frac, color, height=8):
        bar = Gtk.DrawingArea(content_height=height, hexpand=True, valign=Gtk.Align.CENTER)
        bar.set_draw_func(lambda _a, cr, w, h: draw_bar(cr, w, h, frac, color))
        return bar

    def stack(self, parts, total):
        """A bar split into (size, color) parts, like the /tmp breakdown."""
        bar = Gtk.DrawingArea(content_height=10, hexpand=True)

        def draw(_a, cr, w, h):
            rgb(cr, C["line"])
            rounded(cr, 0, 0, w, h, h / 2)
            cr.fill()
            cr.save()
            rounded(cr, 0, 0, w, h, h / 2)
            cr.clip()
            x = 0
            for size, color in parts:
                rgb(cr, color)
                cr.rectangle(x, 0, w * size / total, h)
                cr.fill()
                x += w * size / total
            cr.restore()
        bar.set_draw_func(draw)
        return bar

    def dot(self, color):
        dot = Gtk.DrawingArea(content_width=9, content_height=9, valign=Gtk.Align.CENTER)
        dot.set_draw_func(lambda _a, cr, w, h: draw_dot(cr, w, h, color))
        return dot

    def headline(self, frac, big, sub):
        """The number, then a meter made of cells like the bar's own."""
        top = hbox(label(big, "big"), spacing=10)
        note = label(sub, "sub", expand=True)
        note.set_valign(Gtk.Align.END)
        note.set_margin_bottom(6)
        top.append(note)
        self.body.append(top)
        self.body.append(self.meter(frac, severity(frac)))

    CELLS = 24

    def meter(self, frac, color):
        area = Gtk.DrawingArea(content_height=14, hexpand=True)

        def draw(_a, cr, w, h):
            gap = 3
            cw = (w - gap * (self.CELLS - 1)) / self.CELLS
            lit = round(frac * self.CELLS) if frac > 0 else 0
            for i in range(self.CELLS):
                rgb(cr, color if i < max(lit, 1 if frac > 0 else 0) else C["line"])
                rounded(cr, i * (cw + gap), 0, cw, h, 3)
                cr.fill()
        area.set_draw_func(draw)
        return area

    def columns(self, fracs):
        """One thin vertical bar per value: the cores side by side."""
        area = Gtk.DrawingArea(content_height=44, hexpand=True)

        def draw(_a, cr, w, h):
            gap = 3
            cw = (w - gap * (len(fracs) - 1)) / len(fracs)
            for i, frac in enumerate(fracs):
                x = i * (cw + gap)
                rgb(cr, C["line"])
                rounded(cr, x, 0, cw, h, 2)
                cr.fill()
                fill = max(2, h * min(frac, 1)) if frac > .005 else 0
                if fill:
                    rgb(cr, severity(frac))
                    rounded(cr, x, h - fill, cw, fill, 2)
                    cr.fill()
        area.set_draw_func(draw)
        return area


def severity(frac):
    return C["bad"] if frac >= .9 else C["warn"] if frac >= .75 else C["ok"]


class Procs:
    """Per-program CPU and memory from /proc, summed over a program's
    processes: twenty Firefox content processes read as one Firefox."""
    PAGE = os.sysconf("SC_PAGE_SIZE")

    def __init__(self):
        self.names = {}
        self.prev = {}

    def name(self, pid):
        if pid not in self.names:
            try:
                argv0 = open(f"/proc/{pid}/cmdline", "rb").read().split(b"\0")[0].decode(errors="replace")
                name = os.path.basename(argv0.split()[0]) if argv0 and not argv0.startswith("[") else ""
            except (OSError, IndexError):
                name = ""
            if not name:
                try:
                    name = open(f"/proc/{pid}/comm").read().strip()
                except OSError:
                    name = "?"
            self.names[pid] = name
        return self.names[pid]

    def sample(self):
        """{name: [cpu ticks since the last sample, rss bytes, processes]}."""
        out, ticks = {}, {}
        for pid in os.listdir("/proc"):
            if not pid.isdigit():
                continue
            try:
                stat = open(f"/proc/{pid}/stat").read()
                rss = int(open(f"/proc/{pid}/statm").read().split()[1]) * self.PAGE
            except (OSError, ValueError, IndexError):
                continue
            fields = stat.rsplit(")", 1)[1].split()
            t = int(fields[11]) + int(fields[12])
            ticks[pid] = t
            row = out.setdefault(self.name(pid), [0, 0, 0])
            row[0] += t - self.prev.get(pid, t)
            row[1] += rss
            row[2] += 1
        self.prev = ticks
        self.names = {p: n for p, n in self.names.items() if p in ticks}
        return out


def cpu_times():
    """[(busy, total)] for the whole machine, then each core, from /proc/stat."""
    out = []
    for line in open("/proc/stat"):
        if not line.startswith("cpu"):
            break
        v = [int(x) for x in line.split()[1:]]
        idle = v[3] + v[4]
        out.append((sum(v) - idle, sum(v)))
    return out
