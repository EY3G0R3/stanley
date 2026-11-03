"""Wi-Fi and wired connections."""

from gi.repository import GLib, Gtk

from common import (
    Panel, button, clear, first_available, hbox, label, nm_fields, pill, row, run,
    run_async, vbox,
)


class Network(Panel):
    icon, title, refresh_ms = "󰈀", "Network", 5000

    def __init__(self, app):
        super().__init__(app)
        self.password_for = None
        self.pending = None

    def build_foot(self):
        self.spacer()
        if self.wifi_device():
            self.foot.append(button("Rescan", self.rescan))
        self.foot_button("Settings…", first_available(
            ["dwm-sensible-terminal", "-e", "wlctl"], ["nm-connection-editor"],
            ["dwm-sensible-terminal", "-e", "nmtui"]))

    def wifi_device(self):
        for line in run(["nmcli", "-t", "-f", "DEVICE,TYPE", "dev"]).splitlines():
            f = nm_fields(line)
            if len(f) >= 2 and f[1] == "wifi":
                return f[0]
        return None

    def rescan(self):
        self.sub.set_text("Scanning…")
        run_async(["nmcli", "dev", "wifi", "rescan"], lambda *_: self.refresh())

    def refresh(self):
        if self.password_for:  # do not wipe a password being typed
            return
        clear(self.body)
        devices = []
        for line in run(["nmcli", "-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "dev"]).splitlines():
            f = nm_fields(line)
            if len(f) == 4 and f[1] in ("ethernet", "wifi") and f[2] != "unavailable":
                devices.append(f)
        online = [d for d in devices if d[2] == "connected"]
        self.sub.set_text(", ".join(f"{d[3]} · {d[0]}" for d in online) or "Offline")

        for dev, kind, state, conn in devices:
            if kind == "ethernet":
                ok = state == "connected"
                self.body.append(row("󰈀", conn or dev,
                                     pill(state, "ok" if ok else "")))

        wifi = self.wifi_device()
        if not wifi:
            return
        enabled = run(["nmcli", "radio", "wifi"]).strip() == "enabled"
        sw = Gtk.Switch(active=enabled, valign=Gtk.Align.CENTER)
        sw.connect("state-set", lambda _s, on: self.set_radio(on))
        self.body.append(hbox(label("WI-FI", "sect", expand=True), sw))
        if not enabled:
            return

        saved = {nm_fields(l)[0] for l in run(["nmcli", "-t", "-f", "NAME,TYPE", "con", "show"]).splitlines()
                 if l.endswith(":802-11-wireless")}
        best = {}
        for line in run(["nmcli", "-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY",
                         "dev", "wifi", "list", "--rescan", "no"]).splitlines():
            f = nm_fields(line)
            if len(f) != 4 or not f[1]:
                continue
            inuse, ssid, signal_, sec = f[0] == "*", f[1], int(f[2] or 0), f[3]
            if ssid not in best or inuse or signal_ > best[ssid][1]:
                best[ssid] = (inuse, signal_, sec)
        nets = sorted(best.items(), key=lambda kv: (not kv[1][0], -kv[1][1]))[:10]
        box = vbox(4)
        for ssid, (inuse, strength, sec) in nets:
            bars = "󰤨" if strength > 75 else "󰤥" if strength > 50 else "󰤢" if strength > 25 else "󰤟"
            if ssid == self.pending:
                right = label("connecting…", "meta")
            elif inuse:
                right = pill("connected", "ok")
            else:
                right = label(("󰌾 " if sec else "") + (sec or "open"), "meta")
            box.append(row(bars, ssid, right, current=inuse,
                           on_click=lambda s=ssid, u=inuse, k=sec: self.pick(wifi, s, u, k, saved)))
        self.body.append(box)
        if not nets:
            self.body.append(label("No networks found", "sub"))

    def set_radio(self, on):
        run_async(["nmcli", "radio", "wifi", "on" if on else "off"],
                  lambda *_: GLib.timeout_add(1500, lambda: (self.refresh(), False)[1]))
        return False

    def pick(self, dev, ssid, inuse, sec, saved):
        if inuse:
            self.act(ssid, ["nmcli", "dev", "disconnect", dev])
        elif ssid in saved:
            self.act(ssid, ["nmcli", "con", "up", "id", ssid])
        elif sec:
            self.ask_password(ssid)
        else:
            self.act(ssid, ["nmcli", "dev", "wifi", "connect", ssid])

    def ask_password(self, ssid):
        self.password_for = ssid
        clear(self.body)
        self.body.append(label(f"Password for {ssid}", "sect"))
        entry = Gtk.PasswordEntry(show_peek_icon=True, hexpand=True)
        go = lambda *_: self.connect_with(ssid, entry.get_text())  # noqa: E731
        entry.connect("activate", go)
        self.body.append(entry)
        self.body.append(hbox(Gtk.Box(hexpand=True),
                              button("Cancel", self.cancel_password),
                              button("Connect", go), spacing=8))
        entry.grab_focus()

    def cancel_password(self):
        self.password_for = None
        self.refresh()

    def connect_with(self, ssid, password):
        self.password_for = None
        self.act(ssid, ["nmcli", "dev", "wifi", "connect", ssid, "password", password])

    def act(self, ssid, argv):
        self.pending = ssid
        self.refresh()

        def done(ok, out):
            self.pending = None
            self.refresh()
            if not ok:
                self.sub.set_text(out.strip().splitlines()[-1][:60] if out.strip() else "Failed")
        run_async(argv, done)
