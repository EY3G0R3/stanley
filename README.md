# stanley

Status bar content for [dwl](https://codeberg.org/dwl/dwl). stanley watches
the machine the way Stanley watches the clock, and writes one status line a
second for dwl's bar to draw. Each block on the bar knows its own click, and
the popups that open from them live here too.

One file per machine says which widgets show, where, in what order, and
which parts of each: icon, name, gauge, extra text.

```
section left
hostname   name
date       extra

section center
clock      extra

section right
cpu        icon gauge
disk:/     icon gauge
audio      icon gauge

memory     icon
battery    icon gauge extra
```

A widget that is not listed is never created, so it costs nothing.
`bar.example` documents the format and lists the widgets.

## Your own widgets

A widget of your own is a script and, if it wants one, a popup; neither
touches this repo. The script prints one line, an icon, a tab and the text:

```
script:~/bin/vpn-status   icon extra   click: stanley-popup vpn
```

The popup is `~/.config/stanley/popups/vpn.py`, built from the same parts as
the popups in `src/popups/`:

```python
from common import Panel, label, row


class Popup(Panel):
    icon, title = "\U000f0582", "VPN"

    def refresh(self):
        self.sub.set_text("connected")
        self.body.append(row("\U000f0582", "wg0", label("10.0.0.2")))
```

## Install

Needs Rust, the libpulse headers, gtk4-layer-shell, `wtype` and `yad`.

```
make install    # builds, then links ~/.local/bin/stanley and stanley-popup
stanley --check # validates ~/.config/stanley/bar
```

Run it as dwl's status source: `stanley | dwl`. It speaks the bar protocol of
the [EY3G0R3/dwl](https://github.com/EY3G0R3/dwl) fork: sections, block
markers and color bytes on stdout, clicks back on a FIFO. Update and rebuild
the two together.

See [docs/design.md](docs/design.md) for the design and the protocol.
