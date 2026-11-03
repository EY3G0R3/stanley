# stanley design

stanley is the status bar content for dwl: the blocks on the bar, what their
clicks do, and the popups that open from them. It replaced slstatus, its
`slstatus-action` click router and `slstatus-panel` popups, and the per-host
`hidden` lists. It is named after the clock-watcher in *The Office*,
alongside dwight and sprinkles, and "st..." echoes status.

## Terms

- **Block**: what sits on the bar, its reading and its click
  (`src/blocks/`, Rust). The protocol, dwl and `BLOCK_*` use the same word.
- **Popup**: what opens from a block (`src/popups/`, Python GTK).
- **Widget**: a name in the bar file, covering both, as in `cpu icon gauge`.

## Goals

- One file per machine describes the whole bar: which widgets, in what order,
  where, and which parts of each (icon, name, gauge, extra).
- A block is one source file, with its reading and its click. Adding one does
  not mean touching a format table and a click router.
- Collect only what is shown. An unlisted widget is never constructed, so it
  costs nothing. Detail data (process tables, device lists) belongs to the
  popup that shows it, and is read only while that popup is open.
- No JSON anywhere, and no extra processes per tick.

## Split of work

| Part | Owner |
|---|---|
| Tags, layout symbol, urgency, drawing, click hit-testing, palette | dwl |
| Bar content: blocks, their readings, order and sections, their clicks | stanley |
| Popups (detail views) | stanley, `src/popups/`, run on click |

dwl stays the renderer and owns the palette. stanley sends text, color roles,
block markers and section markers; it never names a color.

## The bar file

`~/.config/stanley/bar`, a yadm alternate per host (`bar##hostname.fractal`).
When it is missing, stanley uses the built-in `bar.example`, which documents
the format and lists the widgets.

```
set meter-style box

section left
hostname   name
date       extra

section center
clock      extra

section right
top        extra

cpu        icon gauge
disk:/     icon gauge
audio      icon gauge

memory     icon
battery    icon gauge extra
```

- **Order** is line order. A widget that is not listed is hidden and never
  constructed.
- **Sections**: `section left|center|right` puts the widgets after it there;
  before any, they are on the right.
- **Groups**: a blank line starts a group. Groups are separated by a wider gap,
  so gauges and bare icons do not mix.
- **Parts**: any of `icon`, `name`, `gauge`, `extra`. No parts means every part
  the widget has. A widget that has nothing to show for the listed parts (the
  mic when it is off) drops out of the bar entirely, gap and all.
- **Argument**: `disk:/home` passes `/home` to the widget.
- **Click**: each block has its own (below). Everything after `click:` is a
  shell command that replaces it, run with `BLOCK_BUTTON`, `BLOCK_X` and
  `BLOCK_WIDTH` set.
- **Settings** (`set key value`): `interval` (ms), `meter-style`,
  `meter-cells`, `warn-at`, `crit-at`, `gap`, `group-gap`, `icon-gap`.

Words split on spaces, so arguments cannot hold one; the clock and date take
no format argument for that reason. The file is read at start and on `SIGHUP`
(`pkill -HUP stanley`); `stanley --check` validates it.

## Blocks

A block fills in a reading each time it is due:

```rust
struct Reading {
    icon: Option<String>,  // a glyph; may change (headset vs HDMI, plug vs battery)
    name: Option<String>,  // short label: "CPU", or the audio card's name
    gauge: Option<Gauge>,  // value 0..100, marked share, severity
    extra: Option<String>, // detail text: process list, percent, fix count
}
```

Text may carry color roles inline (`protocol::paint`); text from outside
goes through `protocol::clean` first. The renderer, not the block, decides
which parts appear, how they are spaced, and how a gauge is drawn. A block
sets its own refresh interval; between refreshes the bar reuses its last
reading. Audio and mic share one PulseAudio connection, kept open.

`Block::click(button)` returns a shell command to run, or does the work
itself and returns none. A click ends the wait and forces its block's next
read, so a volume change shows at once. Buttons count as in X11: 1 left,
2 middle, 3 right, 4 and 5 a wheel notch.

| Block | Left | Other |
|---|---|---|
| cpu, memory, disk, tmp, messages | its popup | |
| battery, wifi, bluetooth, agents | `power`, `network`, `bluetooth`, `agents` popup | |
| audio, mic | `audio` popup | middle: mute; wheel: volume ±2 %, up to 100 % |
| top | the btop scratchpad (`wtype` Super+Escape) | right: `cpu` popup |
| record | stop a recording if one runs, else the `record` popup | |
| clock, date | calendar (`yad`) | |
| hostname | none | |

## Popups

`stanley-popup NAME` opens a popup; calling it again with the same name
closes it, another name replaces it, and Esc or a click outside closes it.
With `BLOCK_X` and `BLOCK_WIDTH` set it is centred under the block, 4 px clear
of either edge; without them it sits at the right edge.

`src/popups/stanley-popup` is the entry point: it preloads gtk4-layer-shell
and re-executes itself, then imports `common.py` (colors, CSS, helpers, cairo
drawing, the `Panel` base, the `Tab` outline, `place()`) and one module per
popup: `network`, `bluetooth`, `audio`, `power`, `tmp`, `agents`, `cpu`,
`memory`, `disk` (all three on `gauge.py`), `messages`, `record`.

## Protocol to dwl

One line per tick on stdout, in three sections, always all three:

```
0x1c <left blocks> 0x1d <center blocks> 0x1e <right blocks>
```

- A block starts with `0x1f` followed by `0x20 + index`, where index is the
  widget's 1-based position in the whole bar file. dwl strips both bytes.
- Role bytes select a color and take no width. dwl maps them through this
  table, which skips 0x0a because a newline ends the line:

| Byte | Role | dwl scheme |
|---|---|---|
| 0x01 | Normal | SchemeNorm |
| 0x02 | Crit | SchemeUrgent |
| 0x03 | Dim | SchemeEmpty |
| 0x04 | Warn | SchemeWarn |
| 0x05 | Ok | SchemeMeterOk |
| 0x06 | Mark | SchemeMeterMark |
| 0x07 | FillEdge | SchemeMeter |
| 0x08 | FillBg | SchemeMeterFill |
| 0x09 | Accent | SchemeMeterAccent |
| 0x0b | BgOk | SchemeToneOk |
| 0x0c | BgWarn | SchemeToneWarn |
| 0x0d | BgCrit | SchemeToneCrit |
| 0x0e | Muted | SchemeMuted |

- Clicks come back on the FIFO `$XDG_RUNTIME_DIR/stanley-click`, six bytes
  each: block index, button, then the pointer's x and the monitor's width as
  little-endian u16, monitor-local logical pixels.
- On a closed stdout stanley dies of `SIGPIPE` (exit 141), which is how
  `dwl-session` tells that dwl has gone. `SIGTERM` and `SIGINT` exit 0.

**How dwl lays it out.** The right section is right aligned, on the selected
monitor only. The center section is centred on the whole bar and drawn last.
The left section is centred between the layout symbol and the center, and is
left out unless it has a bar height of air on both sides. Left and center
draw on every monitor. dwl measures each section with `statusink`, the
function its hit test uses, and keeps each section's x and width for the hit
test.

## Layout

```
stanley/
  Cargo.toml  Makefile  bar.example  README.md
  docs/design.md
  src/
    main.rs        loop, signals, output
    config.rs      the bar file
    render.rs      parts, spacing, groups, sections
    meter.rs       gauge styles
    protocol.rs    block and section markers, color roles
    click.rs       click FIFO and running click commands
    blocks/        one file per block, plus mod.rs (Reading, Block, registry)
    popups/        stanley-popup, common.py, one module per popup
```

Cargo compiles only what `main.rs` reaches, so the Python under `src/` is
ignored by the build. `make` builds; `make install` symlinks
`~/.local/bin/stanley` and `~/.local/bin/stanley-popup` into the checkout.

## Setting up another machine

stanley and dwl speak one protocol, so update and rebuild them together.

1. Install the Rust toolchain, the libpulse headers (Ubuntu: `libpulse-dev`;
   Arch: `libpulse`), gtk4-layer-shell (Ubuntu: `gir1.2-gtk4layershell-1.0`),
   `wtype` and `yad`.
2. `git clone git@github.com:EY3G0R3/stanley.git ~/src/stanley`
3. `cd ~/src/stanley && make install`
4. `make -C ~/src/dwl` with the matching dwl.
5. `stanley --check` exits 0 when the host's bar file is clean. Without a
   `bar##hostname.<host>` file the built-in `bar.example` applies.
6. Log out and in, so dwl and `dwl-session` (yadm) start fresh.

## Next work

**Extension points.** A `script:COMMAND` widget that runs a command at its
interval and shows its output, so a widget can be shared as a script without
touching Rust. `stanley-popup NAME` also looking in
`~/.config/stanley/popups/NAME.py`.

## Known gaps

- `agent-quota` (yadm, `~/.config/systemd/user`) fails its Codex half on
  fractal with `FileNotFoundError`, likely `codex` missing from systemd's
  PATH; its quota is what the agents popup shows.
- The audio device table (`src/blocks/audio.rs`, `DEVICES`) is code. A new
  headset needs a line there.
- Painted meter styles are possible since the role table freed 0x0a, but
  untried on dwl.

## History

1. **Drop-in for `slstatus -s`**, speaking slstatus's protocol so dwl did not
   change; clicks still went through `slstatus-action`.
2. **Sections and the role table.** The clock, date and host name moved from
   dwl into stanley; protocol 1 lines still worked through a `dwl -p`
   handshake.
3. **Popups under their block.** dwl sends the click position.
4. **slstatus retired.** `slstatus-panel` became `src/popups/`,
   `slstatus-action` became block clicks, and protocol 1, dwl's own clock,
   the `slstatus-click` FIFO and the handshake went.
