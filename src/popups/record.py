"""Screen recording: what to record and which sound."""

import array
import ctypes
import fcntl
import glob
import json
import math
import os
import signal
import subprocess

from gi.repository import Gdk, GLib, Gtk

from common import C, Panel, button, clear, draw_bar, label, launch, row, run, vbox


def _die_with_parent():
    # PR_SET_PDEATHSIG: a killed popup must not leave the camera on.
    ctypes.CDLL(None).prctl(1, signal.SIGTERM)


class Preview:
    """A live webcam picture and microphone level while the popup is open.
    ffmpeg streams small JPEG frames and parec raw samples; both stop when
    the popup closes or a recording starts, which needs the devices."""

    def __init__(self):
        self.picture = Gtk.Picture(content_fit=Gtk.ContentFit.COVER,
                                   halign=Gtk.Align.CENTER)
        self.picture.set_size_request(320, 180)
        self.level = Gtk.DrawingArea(content_height=8, hexpand=True,
                                     valign=Gtk.Align.CENTER)
        self.level.set_draw_func(
            lambda _a, cr, w, h: draw_bar(cr, w, h, self.peak, C["ok"]))
        self.procs = {}  # "cam"/"mic" -> (device, Popen, watch id)
        self.frame = b""
        self.peak = 0.0

    def watch(self, kind, device, argv, on_data):
        old = self.procs.get(kind)
        if old and old[0] == device:
            return
        self.stop(kind)
        if device is None:
            return
        proc = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                stdin=subprocess.DEVNULL, preexec_fn=_die_with_parent)
        fd = proc.stdout.fileno()
        fcntl.fcntl(fd, fcntl.F_SETFL, fcntl.fcntl(fd, fcntl.F_GETFL) | os.O_NONBLOCK)

        def readable(_fd, cond):
            try:
                data = os.read(fd, 65536)
            except BlockingIOError:
                return True
            if not data:
                return False
            on_data(data)
            return True

        wid = GLib.io_add_watch(fd, GLib.PRIORITY_DEFAULT, GLib.IO_IN | GLib.IO_HUP, readable)
        self.procs[kind] = (device, proc, wid)

    def show(self, cam, mic):
        """Stream from these devices; None turns one off."""
        if cam != (self.procs.get("cam") or (None,))[0]:
            self.frame = b""
            self.picture.set_paintable(None)
        self.watch("cam", cam, [
            "ffmpeg", "-loglevel", "error", "-f", "v4l2", "-input_format", "mjpeg",
            "-video_size", "640x360", "-framerate", "30", "-i", cam or "",
            "-vf", "fps=15,scale=320:-2", "-f", "mjpeg", "-q:v", "5", "-"], self.frames)
        self.watch("mic", mic, [
            "parec", "-d", mic or "", "--format=s16le", "--channels=1", "--rate=16000",
            "--latency-msec=50", "--raw"], self.samples)
        if mic is None:
            self.peak = 0.0
            self.level.queue_draw()

    def frames(self, data):
        # JPEG data stuffs every 0xFF with 0x00, so FFD9 only ends a frame.
        self.frame += data
        end = self.frame.rfind(b"\xff\xd9")
        if end < 0:
            return
        start = self.frame.rfind(b"\xff\xd8", 0, end)
        if start >= 0:
            try:
                tex = Gdk.Texture.new_from_bytes(GLib.Bytes.new(self.frame[start:end + 2]))
                self.picture.set_paintable(tex)
            except GLib.Error:
                pass
        self.frame = self.frame[end + 2:]

    def samples(self, data):
        data = data[:len(data) // 2 * 2]
        if not data:
            return
        peak = max(abs(v) for v in array.array("h", data)) / 32768
        # -60 dB reads as empty, 0 dB as full; fall back slowly like a meter.
        db = 20 * math.log10(peak) if peak > 0 else -60
        self.peak = max(min(max((db + 60) / 60, 0), 1), self.peak * 0.85)
        self.level.queue_draw()

    def stop(self, kind=None):
        for k in [kind] if kind else list(self.procs):
            entry = self.procs.pop(k, None)
            if not entry:
                continue
            _dev, proc, wid = entry
            GLib.source_remove(wid)
            proc.terminate()
            try:
                proc.wait(1)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()


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
        self.preview = Preview()
        app.connect("shutdown", lambda *_: self.preview.stop())

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
        # A device that is gone (an unplugged webcam) falls back to the default.
        mic = self.opts.get("mic_device")
        if self.opts.get("mic") and mic in dict(self.mics()):
            argv.append("--microphone-device=" + mic)
        cam = self.opts.get("webcam_device")
        if self.opts.get("webcam") and cam in dict(self.cameras()):
            argv.append("--webcam-device=" + cam)
        # The recorder and the webcam overlay open these devices next.
        self.preview.stop()
        # Quit first: the region picker needs the screen this popup covers.
        self.app.quit()
        launch(argv)

    @staticmethod
    def mics():
        """(source name, description) pairs, the default microphone first."""
        try:
            devs = json.loads(run(["pactl", "-f", "json", "list", "sources"]) or "[]")
        except ValueError:
            devs = []
        default = run(["pactl", "get-default-source"]).strip()
        devs = [d for d in devs if not d["name"].endswith(".monitor")]
        devs.sort(key=lambda d: d["name"] != default)
        return [(d["name"], d["description"]) for d in devs]

    @staticmethod
    def cameras():
        """(device, name) pairs. A camera's first node has index 0; its
        others carry metadata. screen-record picks the first by default."""
        cams = []
        for node in sorted(glob.glob("/sys/class/video4linux/video*"),
                           key=lambda n: int(n.rsplit("video", 1)[1])):
            try:
                if open(node + "/index").read().strip() != "0":
                    continue
                name = open(node + "/name").read().strip()
            except OSError:
                continue
            name = name.split(":")[0]
            # One camera can have two capture nodes (RGB and IR); keep the first.
            if name not in {n for _, n in cams}:
                cams.append(("/dev/" + os.path.basename(node), name))
        return cams

    def chosen(self, key, devs):
        """The saved device while it is there, otherwise the first."""
        dev = self.opts.get(key)
        if dev not in dict(devs):
            dev = devs[0][0] if devs else None
        return dev

    def devices(self, key, devs):
        """Rows to choose one device; with no choice saved, the first."""
        chosen = self.chosen(key, devs)
        box = vbox(2)
        for dev, name in devs:
            box.append(row("", name, on_click=lambda d=dev: self.set(key, d),
                           current=dev == chosen))
        return box

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
                                        ("window", "\uf2d0", "Window", "click one"),
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
            if key == "mic" and self.opts.get("mic"):
                box.append(self.devices("mic_device", self.mics()))
            elif key == "webcam" and self.opts.get("webcam"):
                box.append(self.devices("webcam_device", self.cameras()))
        self.body.append(box)

        cam = self.chosen("webcam_device", self.cameras()) if self.opts.get("webcam") else None
        mic = self.chosen("mic_device", self.mics()) if self.opts.get("mic") else None
        self.preview.show(cam, mic)
        if not (cam or mic):
            return
        self.body.append(label("PREVIEW", "sect"))
        box = vbox(8)
        for w in (self.preview.picture, self.preview.level):
            if w.get_parent() is not None:
                w.get_parent().remove(w)
        if cam:
            box.append(self.preview.picture)
        if mic:
            muted = "yes" in run(["pactl", "get-source-mute", mic])
            box.append(row("", "Muted" if muted else "Level", self.preview.level))
        self.body.append(box)
