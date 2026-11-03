"""Bluetooth devices."""

import re

from gi.repository import Gtk

from common import Panel, clear, first_available, hbox, label, pill, row, run, run_async, vbox


class Bluetooth(Panel):
    icon, title, refresh_ms = "󰂯", "Bluetooth", 4000
    ICONS = {"audio-headset": "󰋋", "audio-headphones": "󰋋", "audio-card": "󰓃",
             "input-mouse": "󰍽", "input-keyboard": "󰌌", "input-gaming": "󰊗",
             "phone": "󰏲", "computer": "󰟀"}

    def __init__(self, app):
        super().__init__(app)
        self.pending = set()
        self.switch = Gtk.Switch(valign=Gtk.Align.CENTER)
        self.switch_handler = self.switch.connect("state-set", self.set_power)
        self.extra.append(self.switch)

    def build_foot(self):
        self.spacer()
        self.foot_button("Settings…", first_available(
            ["dwm-sensible-terminal", "-e", "bluetui"], ["blueman-manager"],
            ["dwm-sensible-terminal", "-e", "bluetoothctl"]))

    def set_power(self, _sw, on):
        run_async(["bluetoothctl", "power", "on" if on else "off"], lambda *_: self.refresh())
        return False

    def refresh(self):
        clear(self.body)
        show = run(["bluetoothctl", "show"])
        if "Controller" not in show:
            self.sub.set_text("No Bluetooth adapter")
            self.switch.set_visible(False)
            return
        powered = "Powered: yes" in show
        with self.switch.handler_block(self.switch_handler):
            self.switch.set_active(powered)
        if not powered:
            self.sub.set_text("Off")
            return
        devices = []
        for line in run(["bluetoothctl", "devices", "Paired"]).splitlines():
            parts = line.split(" ", 2)
            if len(parts) == 3 and parts[0] == "Device":
                info = run(["bluetoothctl", "info", parts[1]])
                icon = re.search(r"Icon: (\S+)", info)
                batt = re.search(r"Battery Percentage: \S+ \((\d+)\)", info)
                devices.append((parts[1], parts[2], "Connected: yes" in info,
                                icon.group(1) if icon else "", batt.group(1) if batt else None))
        connected = sum(1 for d in devices if d[2])
        self.sub.set_text(f"{connected} connected" if connected else "On")
        self.body.append(label("PAIRED DEVICES", "sect"))
        box = vbox(4)
        for mac, name, conn, icon, batt in sorted(devices, key=lambda d: not d[2]):
            if mac in self.pending:
                right = label("working…", "meta")
            elif conn:
                right = hbox(label(f"{batt}%", "meta") if batt else None, pill("connected", "ok"), spacing=6)
            else:
                right = label("connect", "meta")
            box.append(row(self.ICONS.get(icon, "󰂱"), name, right, current=conn,
                           on_click=lambda m=mac, c=conn: self.toggle(m, c)))
        self.body.append(box)
        if not devices:
            self.body.append(label("No paired devices", "sub"))

    def toggle(self, mac, connected):
        self.pending.add(mac)
        self.refresh()

        def done(*_):
            self.pending.discard(mac)
            self.refresh()
        run_async(["bluetoothctl", "disconnect" if connected else "connect", mac], done)
