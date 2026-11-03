"""CPU load and the busiest processes."""

from gi.repository import GLib

from common import clear, hbox, label, pill, row, vbox
from gauge import Gauge, Procs, cpu_times


class Cpu(Gauge):
    icon, title = "\uf2db", "CPU"

    def __init__(self, app):
        super().__init__(app)
        self.procs = Procs()
        self.procs.sample()
        self.prev = cpu_times()
        self.first = True

    def build_foot(self):
        self.spacer()
        self.foot_button("btop…", "wtype -M logo -k Escape -m logo")

    def refresh(self):
        if self.first:
            # Usage is a difference between two samples; take the second
            # one shortly rather than a whole refresh period later.
            self.first = False
            GLib.timeout_add(300, lambda: (self.refresh(), False)[1])
            return
        now = cpu_times()
        use = [(b - pb) / (t - pt) if t > pt else 0 for (b, t), (pb, pt) in zip(now, self.prev)]
        elapsed = now[0][1] - self.prev[0][1]
        self.prev = now
        procs = self.procs.sample()

        clear(self.body)
        cores = len(use) - 1
        load = open("/proc/loadavg").read().split()[:3]
        self.sub.set_text(f"{cores} cores · load {' '.join(load)}")
        self.headline(use[0], f"{use[0] * 100:.0f}%", "in use across all cores")

        busiest = max(use[1:])
        self.body.append(hbox(label("CORES", "sect", expand=True),
                              label(f"busiest {busiest * 100:.0f}%", "meta")))
        self.body.append(self.columns(use[1:]))

        # Ticks are per core; elapsed is summed over all of them, so this is
        # each program's share of the whole machine, as the ring is.
        top = sorted(procs.items(), key=lambda kv: -kv[1][0])[:6]
        self.body.append(label("TOP PROGRAMS", "sect"))
        box = vbox(2)
        for name, (t, _rss, n) in top:
            frac = t / elapsed if elapsed else 0
            box.append(row(None, name + (f" ×{n}" if n > 1 else ""),
                           pill(f"{frac * 100:.1f}%", "bad" if frac >= .5 else "warn" if frac >= .2 else "")))
        self.body.append(box)
