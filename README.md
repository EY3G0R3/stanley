# stanley

Status bar content for [dwl](https://codeberg.org/dwl/dwl). stanley watches
the machine the way Stanley watches the clock, and writes one status line a
second for dwl's bar to draw.

One file per machine says which widgets show, in what order, and which parts
of each: icon, name, gauge, extra text.

```
top        extra               click: slstatus-action top

cpu        icon gauge          click: slstatus-action cpu
disk:/     icon gauge          click: slstatus-action disk
audio      icon gauge          click: slstatus-action audio

memory     icon                click: slstatus-action memory
battery    icon gauge extra    click: slstatus-action panel omarchy.power
```

A widget that is not listed is never created, so it costs nothing.
`bar.example` documents the format and lists the widgets.

## Install

Needs Rust and the libpulse headers.

```
make install    # builds, then symlinks ~/.local/bin/stanley
stanley --check # validates ~/.config/stanley/bar
```

Run it as dwl's status source:
`DWL_BAR_PROTOCOL=$(dwl -p) stanley | dwl`. It speaks the bar protocol of
the [EY3G0R3/dwl](https://github.com/EY3G0R3/dwl) fork: sections, block
markers and color bytes on stdout, clicks back on a FIFO. Without
`DWL_BAR_PROTOCOL=2` it speaks protocol 1, the one slstatus speaks, for an
older dwl.

## Status

Phase 2 of three. See [docs/design.md](docs/design.md) for the design, the
protocol, and what comes next.
