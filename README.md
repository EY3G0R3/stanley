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
