"""What every popup shares: colors and CSS, small widget and process helpers,
cairo drawing, the Panel base, the Tab outline, and place().

The entry point, stanley-popup, preloads gtk4-layer-shell before this is
imported; see there.
"""

import math
import os
import re
import shutil
import subprocess

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
gi.require_version("Gtk4LayerShell", "1.0")
gi.require_version("Graphene", "1.0")
from gi.repository import Gdk, Gio, GLib, Graphene, Gtk  # noqa: E402

LOCK_CMD = "if [ -x ~/bin/lock-screen ]; then exec ~/bin/lock-screen; else exec swaylock -f; fi"

# Dracula, matching colors[] in dwl's config.h.
C = dict(bg="#282a36", surface="#2f3241", line="#44475a", fg="#f8f8f2",
         muted="#8b8fa8", accent="#bd93f9", pink="#ff79c6", ok="#50fa7b",
         warn="#f1fa8c", bad="#ff5555", cyan="#8be9fd")

CSS = """
window.root { background: transparent; }
* { font-family: bar, "RecMonoCasual Nerd Font Propo", monospace; font-size: 16pt; color: %(fg)s; }
.pop { padding: 10px 16px 16px; }
.badge {
  min-width: 38px; min-height: 38px; border-radius: 12px;
  background-image: linear-gradient(135deg, %(accent)s, %(pink)s);
}
.badge label { color: %(bg)s; font-size: 1.3em; font-weight: 800; }
.title { font-size: 1.1em; font-weight: 700; }
.sub, .meta { color: %(muted)s; font-size: 0.86em; }
.sect { color: %(muted)s; font-size: 0.79em; letter-spacing: 1px; }
.example { color: %(warn)s; font-size: 0.79em; }
button.row {
  background: transparent; border: 1px solid transparent; border-radius: 12px;
  padding: 7px 10px; box-shadow: none; transition: background 150ms;
}
button.row:hover { background: alpha(%(line)s, .6); }
button.row.cur { background: alpha(%(accent)s, .14); border-color: alpha(%(accent)s, .35); }
box.row { padding: 7px 10px; }
.pill { font-size: 0.79em; padding: 1px 8px; border-radius: 999px; background: %(line)s; }
.pill.ok { background: alpha(%(ok)s, .16); color: %(ok)s; }
.pill.warn { background: alpha(%(warn)s, .14); color: %(warn)s; }
.pill.bad { background: alpha(%(bad)s, .16); color: %(bad)s; }
switch { background: %(line)s; border-radius: 999px; border: none; }
switch:checked { background: %(accent)s; }
switch slider { background: %(fg)s; border-radius: 999px; min-width: 18px; min-height: 18px; margin: 3px; box-shadow: none; border: none; }
scale trough { background: %(line)s; border-radius: 999px; min-height: 8px; }
scale highlight { background-image: linear-gradient(90deg, %(accent)s, %(pink)s); border-radius: 999px; }
scale slider { background: %(fg)s; border: 3px solid %(accent)s; border-radius: 999px; min-width: 14px; min-height: 14px; box-shadow: none; }
button.ic { min-width: 28px; min-height: 28px; padding: 0; border-radius: 8px; background: %(line)s; border: none; box-shadow: none; }
button.ic.muted label { color: %(bad)s; }
.val { color: %(muted)s; font-size: 0.86em; min-width: 40px; }
.foot { border-top: 1px solid %(line)s; padding-top: 12px; }
button.btn {
  background: %(surface)s; border: 1px solid %(line)s; border-radius: 12px;
  padding: 5px 12px; box-shadow: none; font-size: 0.93em;
}
button.btn:hover, button.tile:hover { border-color: %(accent)s; }
button.btn:hover label { color: %(accent)s; }
button.tile {
  background: %(surface)s; border: 1px solid %(line)s; border-radius: 12px;
  padding: 12px 4px 8px; box-shadow: none;
}
.tile .glyph { font-size: 1.4em; }
.tile .name { color: %(muted)s; font-size: 0.86em; }
button.tile.danger:hover { border-color: %(bad)s; }
button.armed { background: alpha(%(bad)s, .16); border-color: %(bad)s; }
button.armed label { color: %(bad)s; }
entry { background: %(surface)s; border: 1px solid %(line)s; border-radius: 10px; padding: 4px 8px; }
entry:focus-within { border-color: %(accent)s; }
.big { font-size: 1.9em; font-weight: 750; }
""" % C


def run(argv, timeout=4):
    """Run a quick query; an absent tool or a failure reads as no output."""
    try:
        return subprocess.run(argv, capture_output=True, text=True,
                              timeout=timeout).stdout
    except (OSError, subprocess.SubprocessError):
        return ""


def run_async(argv, done=None):
    """Run a slow action without freezing the popup; done(ok, output)."""
    try:
        proc = Gio.Subprocess.new(
            argv, Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_MERGE)
    except GLib.Error as err:
        if done:
            done(False, err.message)
        return

    def finish(p, res):
        try:
            _, out, _ = p.communicate_utf8_finish(res)
        except GLib.Error as err:
            out = err.message
        if done:
            done(p.get_successful(), out or "")

    proc.communicate_utf8_async(None, None, finish)


def first_available(*cmds):
    """The first command whose program is installed. The impala-style TUIs
    (wlctl, bluetui) come first, then the GTK settings apps
    Ubuntu ships, then whatever is always there."""
    for cmd in cmds:
        if shutil.which(cmd[2] if cmd[0] == "dwm-sensible-terminal" else cmd[0]):
            return cmd
    return cmds[-1]


def launch(cmd):
    """Start a program detached from the popup, which is about to exit.

    stanley starts before dwl, so the popup inherits no WAYLAND_DISPLAY. GTK
    finds the socket anyway, but kitty then falls back to X11 and fails, so
    hand the child the display this popup is on."""
    env = dict(os.environ)
    env.setdefault("WAYLAND_DISPLAY", Gdk.Display.get_default().get_name())
    subprocess.Popen(cmd, shell=isinstance(cmd, str), start_new_session=True, env=env,
                     stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def human(n):
    for unit, size in (("GiB", 1 << 30), ("MiB", 1 << 20), ("KiB", 1 << 10)):
        if n >= size:
            v = n / size
            return f"{v:.0f} {unit}" if v >= 100 else f"{v:.1f} {unit}"
    return f"{n:.0f} B"


def nm_fields(line):
    """Split nmcli terse output, where ':' inside a value is escaped."""
    return [f.replace("\\:", ":").replace("\\\\", "\\")
            for f in re.split(r"(?<!\\):", line)]


# ---------------------------------------------------------------- widgets

def label(text, css=None, xalign=0.0, expand=False, ellipsize=False):
    w = Gtk.Label(label=text, xalign=xalign, hexpand=expand)
    if css:
        for c in css.split():
            w.add_css_class(c)
    if xalign == 0.0 and not expand:
        w.set_halign(Gtk.Align.START)
    if ellipsize:
        # Take the width the row gives rather than the text's natural
        # width, so a long device name is cut short instead of widening
        # the whole card.
        w.set_ellipsize(3)  # Pango.EllipsizeMode.END
        w.set_width_chars(1)
        w.set_max_width_chars(1)
    return w


def hbox(*children, spacing=10, css=None):
    b = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=spacing)
    if css:
        b.add_css_class(css)
    for c in children:
        if c is not None:
            b.append(c)
    return b


def vbox(spacing=12):
    return Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=spacing)


def pill(text, kind=""):
    return label(text, ("pill " + kind).strip())


def row(icon, name, right=None, on_click=None, current=False):
    """One list line; a button when it does something, plain otherwise."""
    content = hbox(label(icon) if icon else None,
                   label(name, expand=True, ellipsize=True),
                   right if not isinstance(right, str) else label(right, "meta"))
    if on_click is None:
        content.add_css_class("row")
        return content
    b = Gtk.Button(child=content)
    b.add_css_class("row")
    if current:
        b.add_css_class("cur")
    b.connect("clicked", lambda *_: on_click())
    return b


def button(text, on_click, css="btn"):
    b = Gtk.Button(label=text)
    b.add_css_class(css)
    b.connect("clicked", lambda *_: on_click())
    return b


def clear(box):
    while (child := box.get_first_child()) is not None:
        box.remove(child)


def confirm(btn, action, armed_text="Click again"):
    """Make a button that deletes or ends something ask for a second click."""
    original = {}

    def clicked(*_):
        if btn.has_css_class("armed"):
            action()
            return
        btn.add_css_class("armed")
        original["text"] = _button_text(btn)
        _set_button_text(btn, armed_text)

        def disarm():
            btn.remove_css_class("armed")
            _set_button_text(btn, original["text"])
            return False
        GLib.timeout_add(3000, disarm)

    btn.connect("clicked", clicked)


def _text_label(btn):
    child = btn.get_child()
    if isinstance(child, Gtk.Label):
        return child
    return child.get_last_child()  # tiles: glyph above, name below


def _button_text(btn):
    return _text_label(btn).get_text()


def _set_button_text(btn, text):
    _text_label(btn).set_text(text)


# ----------------------------------------------------------------- panels


# ------------------------------------------------------------- drawing

def rgb(cr, hexcolor, alpha=1.0):
    c = Gdk.RGBA()
    c.parse(hexcolor)
    cr.set_source_rgba(c.red, c.green, c.blue, alpha)


def rounded(cr, x, y, w, h, r):
    cr.new_sub_path()
    cr.arc(x + w - r, y + r, r, -math.pi / 2, 0)
    cr.arc(x + w - r, y + h - r, r, 0, math.pi / 2)
    cr.arc(x + r, y + h - r, r, math.pi / 2, math.pi)
    cr.arc(x + r, y + r, r, math.pi, 3 * math.pi / 2)
    cr.close_path()


def draw_ring(cr, w, h, frac, color):
    cx, cy, r = w / 2, h / 2, min(w, h) / 2 - 7
    cr.set_line_width(12)
    rgb(cr, C["line"])
    cr.arc(cx, cy, r, 0, 2 * math.pi)
    cr.stroke()
    if frac > 0:
        cr.set_line_cap(1)  # round
        rgb(cr, color)
        cr.arc(cx, cy, r, -math.pi / 2, -math.pi / 2 + 2 * math.pi * max(frac, .005))
        cr.stroke()
    pct = f"{frac * 100:.0f}%" if frac >= .01 else f"{frac * 100:.1f}%"
    rgb(cr, C["fg"])
    cr.select_font_face("monospace")
    cr.set_font_size(15)
    ext = cr.text_extents(pct)
    cr.move_to(cx - ext.width / 2 - ext.x_bearing, cy - ext.height / 2 - ext.y_bearing)
    cr.show_text(pct)


def draw_dot(cr, w, h, color):
    rgb(cr, color)
    rounded(cr, 0, 0, w, h, 3)
    cr.fill()


def draw_bar(cr, w, h, frac, color):
    rgb(cr, C["line"])
    rounded(cr, 0, 0, w, h, h / 2)
    cr.fill()
    if frac > 0:
        rgb(cr, color)
        rounded(cr, 0, 0, max(h, w * min(frac, 1)), h, h / 2)
        cr.fill()


class Panel:
    icon = "?"
    title = ""
    refresh_ms = 0

    def __init__(self, app):
        self.app = app
        self.sub = label("", "sub")
        self.extra = hbox()
        self.body = vbox()
        self.foot = hbox(spacing=8, css="foot")
        self.foot.set_halign(Gtk.Align.FILL)

    def widget(self):
        badge = Gtk.Box(halign=Gtk.Align.START, valign=Gtk.Align.CENTER, hexpand=False)
        badge.add_css_class("badge")
        glyph = label(self.icon, xalign=0.5, expand=True)
        badge.append(glyph)
        titles = vbox(0)
        titles.set_hexpand(True)
        titles.append(label(self.title, "title"))
        titles.append(self.sub)
        card = vbox(14)
        card.add_css_class("pop")
        card.append(hbox(badge, titles, self.extra, spacing=12))
        card.append(self.body)
        self.build_foot()
        if self.foot.get_first_child() is not None:
            card.append(self.foot)
        self.refresh()
        if self.refresh_ms:
            GLib.timeout_add(self.refresh_ms, lambda: (self.refresh(), True)[1])
        return card

    def foot_button(self, text, cmd):
        b = button(text, lambda: (launch(cmd), self.app.quit()))
        self.foot.append(b)
        return b

    def spacer(self):
        s = Gtk.Box(hexpand=True)
        self.foot.append(s)

    def build_foot(self):
        pass

    def refresh(self):
        pass


class Tab(Gtk.Box):
    """The popup's outline, drawn as an extension of the bar.

    It is filled with the bar's own colour and touches the bar with no gap.
    The top corners curve outwards into the bar (concave fillets of FLARE
    pixels on either side of the card) and the bottom corners are rounded,
    so the card reads as a tab growing out of the bar.
    """
    FLARE = 10
    RADIUS = 12

    def __init__(self, child):
        super().__init__()
        child.set_margin_start(self.FLARE)
        child.set_margin_end(self.FLARE)
        self.append(child)

    def do_snapshot(self, snapshot):
        w, h, f, r = self.get_width(), self.get_height(), self.FLARE, self.RADIUS
        cr = snapshot.append_cairo(Graphene.Rect().init(0, 0, w, h))
        cr.move_to(0, 0)
        cr.arc(0, f, f, -math.pi / 2, 0)                     # left flare
        cr.line_to(f, h - r)
        cr.arc_negative(f + r, h - r, r, math.pi, math.pi / 2)
        cr.line_to(w - f - r, h)
        cr.arc_negative(w - f - r, h - r, r, math.pi / 2, 0)
        cr.line_to(w - f, f)
        cr.arc(w, f, f, math.pi, 3 * math.pi / 2)            # right flare
        cr.close_path()
        rgb(cr, C["bg"])
        cr.fill_preserve()
        # An outline on everything but the edge the bar continues into, so
        # the tab still shows over dwl's background, which is the same colour.
        cr.new_path()
        cr.arc(0, f, f, -math.pi / 2, 0)
        cr.line_to(f, h - r)
        cr.arc_negative(f + r, h - r, r, math.pi, math.pi / 2)
        cr.line_to(w - f - r, h)
        cr.arc_negative(w - f - r, h - r, r, math.pi / 2, 0)
        cr.line_to(w - f, f)
        cr.arc(w, f, f, math.pi, 3 * math.pi / 2)
        cr.set_line_width(2)
        rgb(cr, C["line"])
        cr.stroke()
        Gtk.Box.do_snapshot(self, snapshot)


def place(reveal, card):
    """Centre the card under the clicked block, 4 px clear of either edge.

    stanley sets BLOCK_X and BLOCK_WIDTH, the pointer's x and the monitor's
    width in logical pixels, from the click dwl reported. Without them (an
    older dwl or bar) the card stays at the right edge.
    """
    try:
        x, width = int(os.environ["BLOCK_X"]), int(os.environ["BLOCK_WIDTH"])
    except (KeyError, ValueError):
        reveal.set_halign(Gtk.Align.END)
        reveal.set_margin_end(4)
        return
    natural = card.measure(Gtk.Orientation.HORIZONTAL, -1)[1]
    reveal.set_halign(Gtk.Align.START)
    reveal.set_margin_start(max(4, min(x - natural // 2, width - natural - 4)))
