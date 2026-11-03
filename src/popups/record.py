"""Screen recording: what to record and which sound."""

import json
import os

from gi.repository import Gtk

from common import Panel, button, clear, label, launch, row, vbox


class Record(Panel):
    """Screen recording through ~/bin/screen-record: what to capture and
    with which sound, then Record. A click on the bar block stops a
    recording that runs, so this popup only ever starts one."""
    icon, title = "\U000f0ec2", "Screen recording"
    OPTIONS = os.path.expanduser("~/.local/state/screen-record/options.json")
    SOURCES = (("audio", "\U000f057e", "Desktop audio"), ("mic", "", "Microphone"),
               ("webcam", "", "Webcam overlay"))

    def __init__(self, app):
        super().__init__(app)
        try:
            self.opts = json.load(open(self.OPTIONS))
        except (OSError, ValueError):
            # Alt+Print's choice: a region, with everything on.
            self.opts = {"mode": "region", "audio": True, "mic": True, "webcam": True}

    def save(self):
        os.makedirs(os.path.dirname(self.OPTIONS), exist_ok=True)
        with open(self.OPTIONS, "w") as f:
            json.dump(self.opts, f)

    def build_foot(self):
        self.foot_button("Open folder", ["xdg-open", os.path.expanduser("~/Videos")])
        self.spacer()
        self.foot.append(button("\U000f0ec2  Record", self.start))

    def start(self):
        argv = [os.path.expanduser("~/bin/screen-record"), "--toggle",
                "--" + self.opts.get("mode", "region")]
        argv += [f"--{k}" for k in ("audio", "mic", "webcam") if self.opts.get(k)]
        # Quit first: the region picker needs the screen this popup covers.
        self.app.quit()
        launch(argv)

    def set(self, key, value):
        self.opts[key] = value
        self.save()
        self.refresh()

    def refresh(self):
        self.sub.set_text("Saved to ~/Videos")
        clear(self.body)
        self.body.append(label("CAPTURE", "sect"))
        box = vbox(2)
        mode = self.opts.get("mode", "region")
        for key, glyph, name, about in (("region", "\U000f0a87", "Region", "drag, or click"),
                                        ("window", "\uf2d0", "Window", "the focused one"),
                                        ("fullscreen", "\U000f0379", "Screen", "pick one")):
            box.append(row(glyph, name, about, on_click=lambda k=key: self.set("mode", k),
                           current=mode == key))
        self.body.append(box)
        self.body.append(label("SOUND AND CAMERA", "sect"))
        box = vbox(2)
        for key, glyph, name in self.SOURCES:
            sw = Gtk.Switch(valign=Gtk.Align.CENTER, active=bool(self.opts.get(key)))
            sw.connect("state-set", lambda _s, on, k=key: (self.set(k, on), False)[1])
            box.append(row(glyph, name, sw))
        self.body.append(box)
