"""The monitor's inputs: hand the screen to another machine, and maybe sleep."""

import os
import re

from gi.repository import Gtk

from common import Panel, label, launch, run_async, vbox

# Each input's DDC value lives in the ~/bin script that switches to it.
INPUTS = (("\U000f0841", "HDMI", "hdmi"),
          ("\U000f0553", "USB-C", "usbc"),
          ("\U000f0379", "DisplayPort", "displayport"))
SWITCH = os.path.expanduser("~/bin/switch-display-to-")
SLEEP = os.path.expanduser("~/bin/switch-display-and-sleep")
# Inputs with nothing plugged in, one name per line; DDC cannot tell.
UNPLUGGED = os.path.join(os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config"),
                         "stanley", "kvm-unplugged")


def unplugged():
    try:
        with open(UNPLUGGED) as f:
            return {line.strip() for line in f if line.strip()}
    except OSError:
        return set()


def ddc_value(arg):
    """The input's 0x60 value, read from its switch script's spec, as an int."""
    try:
        with open(SWITCH + arg) as f:
            m = re.search(r"\|0x([0-9a-fA-F]+)\|", f.read())
    except OSError:
        return None
    return int(m.group(1), 16) if m else None


class Kvm(Panel):
    icon, title = "\U000f1483", "KVM"

    def refresh(self):
        self.sub.set_text("Send the monitor to another input")
        self.unplugged = unplugged()
        self.tiles = {}
        self.section("SWITCH", lambda arg: [SWITCH + arg], self.tiles)
        self.section("SUSPEND AND SWITCH", lambda arg: [SLEEP, arg], {})
        # About a second over DDC, so the tiles show first and the mark follows.
        run_async(["ddcutil", "getvcp", "60", "--terse"], self.mark_current)

    def section(self, title, argv, tiles):
        self.body.append(label(title, "sect"))
        grid = Gtk.Grid(column_spacing=8, column_homogeneous=True)
        for i, (glyph, name, arg) in enumerate(INPUTS):
            inner = vbox(4)
            inner.append(label(glyph, "glyph", xalign=0.5))
            inner.append(label(name, "name", xalign=0.5))
            b = Gtk.Button(child=inner)
            b.add_css_class("tile")
            b.set_sensitive(arg not in self.unplugged)
            b.connect("clicked", lambda *_, cmd=argv(arg): (self.app.quit(), launch(cmd)))
            grid.attach(b, i, 0, 1, 1)
            tiles[arg] = b
        self.body.append(grid)

    def mark_current(self, ok, out):
        """Highlight the input the monitor shows now, under SWITCH only."""
        m = re.search(r"VCP 60 SNC x([0-9a-fA-F]+)", out) if ok else None
        if not m:
            return
        now = int(m.group(1), 16)
        for glyph, name, arg in INPUTS:
            if ddc_value(arg) == now:
                self.tiles[arg].add_css_class("cur")
                self.sub.set_text(f"On {name} now")
