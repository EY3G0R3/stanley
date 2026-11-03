"""Audio outputs, inputs and volume."""

import json
import os

from gi.repository import Gtk

from common import Panel, clear, hbox, label, row, run, run_async, vbox

# The same switch win+[ and friends use: one card on, the others off.
SET_CARD = os.path.expanduser("~/bin/igoraudio-set-card")


class Audio(Panel):
    icon, title = "󰕾", "Audio"

    def build_foot(self):
        self.spacer()
        self.foot_button("Mixer…", ["pavucontrol"])

    @staticmethod
    def volume(dev):
        vals = [int(ch["value_percent"].rstrip("%")) for ch in dev.get("volume", {}).values()]
        return round(sum(vals) / len(vals)) if vals else 0

    @staticmethod
    def glyph(dev):
        name = dev["name"]
        if name.startswith("bluez"):
            return "󰋋"
        if "hdmi" in name:
            return "󰍹"
        return "󰍬" if dev.get("_kind") == "source" else "󰓃"

    def devices(self, kind):
        try:
            devs = json.loads(run(["pactl", "-f", "json", "list", kind + "s"]) or "[]")
        except ValueError:
            devs = []
        devs = [d for d in devs if not d["name"].endswith(".monitor")]
        for d in devs:
            d["_kind"] = kind
        default = run(["pactl", "get-default-" + kind]).strip()
        return devs, default

    @staticmethod
    def outputs():
        """Output cards as (role, card name, description), for igoraudio-set-card.

        Switching turns the other cards off, so a card can have no sink yet;
        list cards, not sinks, or an off card could never be picked."""
        try:
            cards = json.loads(run(["pactl", "-f", "json", "list", "cards"]) or "[]")
        except ValueError:
            cards = []
        found = []
        for c in cards:
            profiles = [p for p in c.get("profiles", {}) if p != "off"]
            if all(p.startswith("input:") for p in profiles):
                continue  # webcams and other input-only cards
            props = c.get("properties", {})
            if props.get("device.bus") == "bluetooth":
                role = "bluetooth"
            elif any("hdmi" in p.lower() for p in profiles):
                role = "hdmi"
            else:
                role = "headphones"
            found.append((role, c["name"], props.get("device.description", c["name"])))
        return found

    def slider(self, kind, dev):
        mute = Gtk.Button(label=("󰝟" if dev["mute"] else "󰕾") if kind == "sink"
                          else ("󰍭" if dev["mute"] else "󰍬"))
        mute.add_css_class("ic")
        if dev["mute"]:
            mute.add_css_class("muted")
        mute.connect("clicked", lambda *_: run_async(
            ["pactl", f"set-{kind}-mute", dev["name"], "toggle"], lambda *_: self.refresh()))
        scale = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 100, 1)
        scale.set_hexpand(True)
        scale.set_draw_value(False)
        scale.set_value(self.volume(dev))
        val = label(f"{self.volume(dev)}%", "val", xalign=1.0)

        def changed(s):
            v = int(s.get_value())
            val.set_text(f"{v}%")
            run_async(["pactl", f"set-{kind}-volume", dev["name"], f"{v}%"])
        scale.connect("value-changed", changed)
        return hbox(mute, scale, val)

    def refresh(self):
        clear(self.body)
        sinks, dsink = self.devices("sink")
        sources, dsource = self.devices("source")
        cur = next((s for s in sinks if s["name"] == dsink), None)
        self.sub.set_text(f"Output · {cur['description']}" if cur else "No output")
        if cur:
            self.body.append(self.slider("sink", cur))
        src = next((s for s in sources if s["name"] == dsource), None)
        if src:
            self.body.append(self.slider("source", src))
        outputs = self.outputs()
        if outputs:
            self.body.append(label("OUTPUT", "sect"))
            box = vbox(4)
            live = cur["properties"].get("device.name") if cur else None
            glyphs = {"bluetooth": "󰋋", "hdmi": "󰍹", "headphones": "󰓃"}
            for role, card, desc in outputs:
                box.append(row(glyphs[role], desc, role.upper(), current=card == live,
                               on_click=lambda r=role: run_async(
                                   [SET_CARD, r], lambda *_: self.refresh())))
            self.body.append(box)
        for heading, kind, devs, default in (("INPUT", "source", sources, dsource),):
            if len(devs) < 1:
                continue
            self.body.append(label(heading, "sect"))
            box = vbox(4)
            for d in devs:
                box.append(row(self.glyph(d), d["description"], f"{self.volume(d)}%",
                               current=d["name"] == default,
                               on_click=lambda n=d["name"], k=kind: run_async(
                                   ["pactl", f"set-default-{k}", n], lambda *_: self.refresh())))
            self.body.append(box)
