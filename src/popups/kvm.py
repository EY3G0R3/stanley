"""The monitor's inputs: hand the screen to another machine, and maybe sleep."""

import os

from gi.repository import Gtk

from common import Panel, label, launch, vbox

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


class Kvm(Panel):
    icon, title = "\U000f1483", "KVM"

    def refresh(self):
        self.sub.set_text("Send the monitor to another input")
        self.unplugged = unplugged()
        self.section("SWITCH", lambda arg: [SWITCH + arg])
        self.section("SUSPEND AND SWITCH", lambda arg: [SLEEP, arg])

    def section(self, title, argv):
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
        self.body.append(grid)
