"""Filesystems and how full they are."""

import os
import time


from common import clear, first_available, hbox, human, label, vbox
from gauge import Gauge, severity


class Disk(Gauge):
    icon, title = "\uf1c0", "Disk"
    SKIP = ("loop", "ram", "zram", "dm-", "sr")

    def __init__(self, app):
        super().__init__(app)
        self.prev = None

    def build_foot(self):
        self.foot_button("Disks…", ["gnome-disks"])
        self.spacer()
        self.foot_button("What uses space…", first_available(
            ["dwm-sensible-terminal", "-e", "dua", "interactive", os.path.expanduser("~")],
            ["dwm-sensible-terminal", "-e", "ncdu", os.path.expanduser("~")]))

    @staticmethod
    def filesystems():
        """One (mount, size, used, free) per block device, at its shortest
        mount point: btrfs mounts one filesystem many times as subvolumes."""
        seen = {}
        for line in open("/proc/self/mounts"):
            dev, mount = line.split()[:2]
            if not dev.startswith("/dev/") or dev.startswith(("/dev/loop", "/dev/zram")):
                continue
            mount = mount.replace("\\040", " ")
            if dev in seen and len(seen[dev]) <= len(mount):
                continue
            seen[dev] = mount
        out = []
        for mount in sorted(seen.values(), key=lambda m: (m != "/", m)):
            try:
                st = os.statvfs(mount)
            except OSError:
                continue
            size = st.f_blocks * st.f_frsize
            if size:
                out.append((mount, size, (st.f_blocks - st.f_bfree) * st.f_frsize, st.f_bavail * st.f_frsize))
        return out

    def io(self):
        """Bytes read and written per second across whole disks."""
        sectors, now = [0, 0], time.monotonic()
        for line in open("/proc/diskstats"):
            f = line.split()
            name = f[2]
            if name.startswith(self.SKIP) or not os.path.exists(f"/sys/block/{name}"):
                continue
            sectors[0] += int(f[5])
            sectors[1] += int(f[9])
        prev, self.prev = self.prev, (sectors, now)
        if not prev:
            return None
        dt = now - prev[1]
        return [(s - p) * 512 / dt for s, p in zip(sectors, prev[0])]

    def refresh(self):
        rates = self.io()
        clear(self.body)
        fss = self.filesystems()
        if not fss:
            self.body.append(label("No filesystems found", "sub"))
            return
        mount, size, used, free = fss[0]
        self.sub.set_text(f"{mount} · " + (f"read {human(rates[0])}/s · write {human(rates[1])}/s"
                                          if rates is not None else "measuring I/O…"))
        self.headline(used / size, human(free), f"free of {human(size)} on {mount}")
        if len(fss) > 1:
            self.body.append(label("OTHER FILESYSTEMS", "sect"))
            box = vbox(8)
            for mount, size, used, free in fss[1:]:
                box.append(hbox(label(mount, ellipsize=True, expand=True),
                                label(f"{human(free)} free of {human(size)}", "meta"), spacing=8))
                box.append(self.bar(used / size, severity(used / size)))
            self.body.append(box)
