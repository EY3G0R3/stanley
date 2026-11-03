"""Session actions and batteries."""

import os
import re
import signal

from gi.repository import Gtk

from common import LOCK_CMD, Panel, clear, confirm, label, launch, pill, row, run, vbox


class Power(Panel):
    icon, title = "⏻", "Session"

    def refresh(self):
        clear(self.body)
        try:
            up = float(open("/proc/uptime").read().split()[0])
        except OSError:
            up = 0
        d, h = int(up // 86400), int(up % 86400 // 3600)
        self.sub.set_text(f"{os.environ.get('USER', '')} · up " + (f"{d}d {h}h" if d else f"{h}h {int(up % 3600 // 60)}m"))

        grid = Gtk.Grid(column_spacing=8, row_spacing=8, column_homogeneous=True)
        tiles = (("󰌾", "Lock", LOCK_CMD, False),
                 ("󰤄", "Suspend", "systemctl suspend", False),
                 ("󰒲", "Hibernate", "systemctl hibernate", False),
                 ("󰍃", "Log out", self.logout, True),
                 ("󰜉", "Reboot", "systemctl reboot", True),
                 ("⏻", "Shut down", "systemctl poweroff", True))
        for i, (glyph, name, cmd, danger) in enumerate(tiles):
            inner = vbox(4)
            inner.append(label(glyph, "glyph", xalign=0.5))
            inner.append(label(name, "name", xalign=0.5))
            b = Gtk.Button(child=inner)
            b.add_css_class("tile")
            act = (lambda c=cmd: self.do(c))
            if danger:
                b.add_css_class("danger")
                confirm(b, act)
            else:
                b.connect("clicked", lambda *_, a=act: a())
            grid.attach(b, i % 3, i // 3, 1, 1)
        self.body.append(grid)

        batteries = []
        for path in run(["upower", "-e"]).split():
            if path.endswith("DisplayDevice"):
                continue
            info = run(["upower", "-i", path])
            pct = re.search(r"percentage:\s+(\d+)", info)
            model = re.search(r"model:\s+(.+)", info)
            if pct:
                batteries.append(((model.group(1).strip() if model else path.rsplit("/", 1)[-1]), int(pct.group(1))))
        if batteries:
            self.body.append(label("BATTERIES", "sect"))
            box = vbox(4)
            for name, pct in batteries:
                kind = "bad" if pct <= 15 else "warn" if pct <= 30 else "ok"
                box.append(row("󰁹", name, pill(f"{pct}%", kind)))
            self.body.append(box)

    def do(self, cmd):
        self.app.quit()
        cmd() if callable(cmd) else launch(cmd)

    @staticmethod
    def logout():
        """SIGTERM the dwl this popup runs under; dwl quits cleanly on it."""
        pid = os.getppid()
        while pid > 1:
            try:
                comm = open(f"/proc/{pid}/comm").read().strip()
                stat = open(f"/proc/{pid}/stat").read()
            except OSError:
                return
            if comm == "dwl":
                os.kill(pid, signal.SIGTERM)
                return
            pid = int(stat.rsplit(")", 1)[1].split()[1])
