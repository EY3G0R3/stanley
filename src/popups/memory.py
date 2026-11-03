"""Memory use and the biggest processes."""


from common import C, clear, hbox, human, label, row, vbox
from gauge import Gauge, Procs


class Memory(Gauge):
    icon, title = "\ue266", "Memory"

    def __init__(self, app):
        super().__init__(app)
        self.procs = Procs()

    def build_foot(self):
        self.spacer()
        self.foot_button("btop…", "wtype -M logo -k Escape -m logo")

    def refresh(self):
        info = {}
        for line in open("/proc/meminfo"):
            key, value = line.split(":", 1)
            info[key] = int(value.split()[0]) * 1024
        total, avail, free = info["MemTotal"], info["MemAvailable"], info["MemFree"]
        used, cache = total - avail, avail - free

        clear(self.body)
        self.sub.set_text(f"{human(total)} installed")
        self.headline(used / total, human(avail), f"available · {human(used)} used by programs")

        parts = (("Programs", used, C["accent"]), ("Cache, can be reclaimed", cache, C["cyan"]),
                 ("Free", free, C["line"]))
        self.body.append(self.stack([(size, color) for _n, size, color in parts], total))
        legend = vbox(2)
        for name, size, color in parts:
            legend.append(hbox(self.dot(color), label(name, expand=True), label(human(size), "meta"), spacing=8))
        if info.get("SwapTotal"):
            swap = info["SwapTotal"] - info["SwapFree"]
            legend.append(hbox(self.dot(C["warn"] if swap else C["line"]), label("Swap in use", expand=True),
                               label(f"{human(swap)} of {human(info['SwapTotal'])}", "meta"), spacing=8))
        self.body.append(legend)

        top = sorted(self.procs.sample().items(), key=lambda kv: -kv[1][1])[:6]
        self.body.append(label("TOP PROGRAMS", "sect"))
        box = vbox(2)
        for name, (_t, rss, n) in top:
            box.append(row(None, name + (f" ×{n}" if n > 1 else ""), label(human(rss), "meta")))
        self.body.append(box)
