# stanley design

stanley is the status bar content for dwl. It replaces slstatus, the
`slstatus-action` click router and the per-host `hidden` lists. It is named
after the clock-watcher in *The Office*, alongside dwight and sprinkles, and
"st..." echoes status.

## Goals

- One file per machine describes the whole bar: which widgets, in what order,
  and which parts of each (icon, name, gauge, extra).
- A widget is one source file. Adding one does not mean touching a list of
  functions, a format table and a click router.
- Collect only what is shown. An unlisted widget is never constructed, so it
  costs nothing. Detail data (process tables, device lists) belongs to the
  popup that shows it, and is read only while that popup is open.
- No JSON anywhere, and no extra processes per tick.

## Split of work

| Part | Owner |
|---|---|
| Tags, layout symbol, urgency, drawing, click hit-testing, palette | dwl |
| Bar content: widgets, their readings, their order | stanley |
| Popups (detail views) | Python GTK, run on click |

dwl stays the renderer and owns the palette. stanley sends text, color roles
and block markers; it never names a color.

## Phases

1. **Drop-in replacement for `slstatus -s`.** Same protocol to dwl, so dwl does
   not change. The bar file replaces `config.def.h` composition and `hidden`.
   Click commands come from the bar file; they still call `slstatus-action`.
   Switching is one line in `dwl-session`, and switching back is as easy.
2. **dwl protocol v2.** Sections (left, center, right) so the clock can stay
   centred; clock, date and hostname move from dwl into stanley as widgets;
   dwl maps roles through a documented table instead of its `Scheme` enum.
3. **Popups move in.** `slstatus-panel` becomes `panels/` here, one module per
   widget, and `slstatus-action` routing is retired.

This document plans phase 1 in detail. "Status and next work" at the end
sketches what comes next, enough to pick the work up on another machine.

## The bar file

`~/.config/stanley/bar`, a yadm alternate per host (`bar##hostname.fractal`).
When it is missing, stanley uses the built-in `bar.example`.

```
# set <key> <value> lines first, then one widget per line in bar order.
set meter-style box

top       extra                  click: slstatus-action top

cpu       icon gauge             click: slstatus-action cpu
disk      icon gauge             click: slstatus-action disk
audio     icon gauge             click: slstatus-action audio
mic       extra                  click: slstatus-action audio

memory    icon                   click: slstatus-action memory
tmp       icon                   click: slstatus-action panel igorg.tmp-quota
record    icon                   click: slstatus-action record
```

- **Order** is line order. A widget that is not listed is hidden and never
  constructed.
- **Groups**: a blank line starts a group. Groups are separated by a wider gap,
  so gauges and bare icons do not mix.
- **Parts**: any of `icon`, `name`, `gauge`, `extra`. No parts means every part
  the widget has. A widget that has nothing to show for the listed parts (the
  mic when it is off) drops out of the bar entirely, gap and all.
- **Argument**: `disk:/home` passes `/home` to the widget.
- **Click**: everything after `click:` is a shell command, run with
  `BLOCK_BUTTON` set, as slstatus did.
- **Settings** (`set key value`): `interval` (ms), `meter-style`,
  `meter-cells`, `warn-at`, `crit-at`, `gap`, `group-gap`, `icon-gap`.

The file is read at start and on `SIGHUP` (`pkill -HUP stanley`).

## Widgets

A widget fills in a reading each time it is due:

```rust
struct Reading {
    icon: Option<String>,  // a glyph; may change (headset vs HDMI, plug vs battery)
    name: Option<String>,  // short label: "CPU", or the audio card's name
    gauge: Option<Gauge>,  // value 0..100, marked share, severity
    extra: Option<String>, // detail text: process list, percent, fix count
}
```

Text may carry color roles inline (`protocol::paint`); text from outside
goes through `protocol::clean` first. The renderer, not the widget, decides
which parts appear, how they are spaced, and how a gauge is drawn. A widget
sets its own refresh interval; between refreshes the bar reuses its last
reading.

Phase 1 widgets (parity with today's bar): `top`, `cpu`, `memory`, `disk`,
`tmp`, `audio`, `mic`, `record`, `agents`, `messages`, `bluetooth`, `wifi`,
`battery`.

Audio keeps one PulseAudio connection open instead of reconnecting every
second.

## Protocol to dwl (phase 1: unchanged from slstatus)

One line per tick on stdout.

- A block starts with `0x1f` followed by `0x20 + index`, where index is the
  widget's 1-based position in the bar file. dwl strips both bytes.
- Bytes `0x01`..`0x0f` select a color role. Phase 1 roles are dwl's current
  `Scheme` indices, listed in `src/protocol.rs`.
- Clicks arrive as two bytes (block index, button) on the FIFO
  `$SLSTATUS_CLICK_FIFO`, else `$XDG_RUNTIME_DIR/slstatus-click`. That is the
  path dwl writes today; phase 2 renames it.
- On a closed stdout stanley dies of `SIGPIPE` (exit 141), which is how
  `dwl-session` tells that dwl has gone. `SIGTERM` and `SIGINT` exit 0.

## Layout

```
stanley/
  Cargo.toml  Makefile  bar.example
  docs/design.md
  src/
    main.rs        loop, signals, output
    config.rs      the bar file
    render.rs      parts, spacing, groups
    meter.rs       gauge styles
    protocol.rs    block markers and color roles
    click.rs       click FIFO and dispatch
    widgets/       one file per widget, plus mod.rs (Reading, registry)
```

`make` builds; `make install` symlinks the binary into `~/.local/bin`.

## Phase 1 acceptance

- With a bar file matching fractal's current bar, stanley's output draws the
  same blocks in the same order with the same colors as slstatus.
- Clicks on every block run the configured command.
- `pkill -HUP stanley` re-reads the bar file; `pkill stanley` restarts it
  through `dwl-session`.
- An unlisted widget is never constructed (audio without `audio` or `mic`
  opens no PulseAudio connection).

## Status and next work

As of 2026-11-02 PST.

**Done: phase 1.** stanley is the live bar on fractal. `~/bin/dwl-session`
(yadm) runs `stanley` where it is installed and `slstatus -s` elsewhere.
Bar files are yadm alternates in `~/.config/stanley/`.

### Setting up another machine

1. Install the Rust toolchain and libpulse headers (Ubuntu: `libpulse-dev`;
   Arch: `libpulse`).
2. `git clone git@github.com:EY3G0R3/stanley.git ~/src/stanley`
3. `cd ~/src/stanley && make install` builds and symlinks `~/.local/bin/stanley`.
4. Check the host's bar file: `stanley --check` exits 0 when it is clean.
   Without a `bar##hostname.<host>` file the built-in `bar.example` applies.
5. Restart dwl (log out and in), or `pkill -x slstatus` once `dwl-session`
   from yadm is current: its loop picks stanley on the next start.

### Next 1: popups open under their block

**Problem.** Every popup opens at the right edge, even for a block on the
left. `slstatus-panel` aligns its card with `halign=END` and never learns
where the click was: dwl sends only the block and the button.

**Fix.** A click message that carries the position, on a new FIFO so old and
new ends never misread each other:

- dwl (`sendstatusclick`, `src/dwl/dwl.c`) writes 6 bytes to
  `$XDG_RUNTIME_DIR/stanley-click`: block, button, then x and the monitor
  width as little-endian u16, both monitor-local logical pixels
  (`cursor->x - selmon->m.x` and `selmon->m.width`). If that FIFO cannot be
  opened (no stanley reading it), it writes the old 2 bytes to
  `slstatus-click`, so slstatus keeps working.
- stanley (`src/click.rs`) reads both FIFOs while old dwl builds are
  around, and runs the click command with `BLOCK_X` and `BLOCK_WIDTH` set.
  The scroll path in dwl (`axisnotify`) sends the same message.
- `slstatus-panel` reads `BLOCK_X` and `BLOCK_WIDTH`. With them, the card
  goes `halign=START` with `margin_start` = x minus half the card's natural
  width, clamped to 4 px from either edge. Without them, it stays right
  aligned as now. `slstatus-action` passes the environment through.

**Check.** Click CPU on the left: the popup opens under it. Click the
battery at the right edge: the popup stays on screen.

### Next 2: phase 2, dwl protocol v2

- **Sections.** A section byte before blocks (left, center, right), so dwl can
  lay out tags, then stanley's left section, the center section in the
  middle, and the right section at the end.
- **Clock, date, hostname** become stanley widgets, so each host can show
  them as icon or text, or not at all. dwl's own clock code (`drawbar`,
  `clockx`, `datex`) goes; its clock click becomes an ordinary block click.
- **Role table.** dwl maps role bytes through a fixed, documented table
  instead of indexing its `Scheme` enum. That also frees 0x0a (a newline)
  so the painted meter styles can work.
- **Click FIFO rename** is done by Next 1.

#### Phase 2 plan

**Byte values.** A v2 line is three sections, always all three, in order:

```
0x1c <left blocks> 0x1d <center blocks> 0x1e <right blocks>
```

Inside a section, blocks are as in phase 1: `0x1f`, then `0x20 + index`,
then text. The index is still the widget's position in the whole bar file,
so clicks and the click FIFO do not change. Role bytes go through this
table, which skips 0x0a:

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

**Bar file.** `section left`, `section center` and `section right` start a
section; widgets before any `section` line are on the right, so phase 1 bar
files still work. New widgets: `clock` (`clock:%H:%M` takes a strftime
format, default `%I:%M %p`), `date` (default `%Y-%m-%d`) and `hostname`.

**How dwl lays it out.** dwl splits the line into the three sections when it
reads it. The right section is drawn as the status is today: right aligned,
on the selected monitor only. The center section is centred on the whole bar
and drawn last, as the clock is today. The left section is centred in the
space between the layout symbol and the center section, as the date and host
name are today, and is left out unless it has a bar height of air on both
sides. Left and center draw on every monitor, as the clock does. Each width
is measured with `statusink`, the function the hit test uses, and dwl keeps
each section's x and width for the hit test.

**Coexistence.** stanley is piped into dwl, not started by it, so dwl cannot
set its environment. Instead, `dwl -p` prints the bar protocol it reads
(`2`). `dwl-session` and `dwl-nested` export `DWL_BAR_PROTOCOL` from it
before starting the bar.

- New dwl, new stanley: v2.
- Old dwl: `-p` is an unknown option, the variable is empty, and stanley
  writes phase 1 lines. Left and center widgets are left out, since old dwl
  draws its own clock.
- New dwl, slstatus or old stanley: a line with no section byte is v1. dwl
  indexes `Scheme` and draws its own clock, date and host name as before.
  That code goes once slstatus is retired.

### Next 3: phase 3, popups move in

`slstatus-panel` becomes `panels/` in this repo, one module per widget, run
on click. `slstatus-action` routing is replaced by the click column, so a
bar line says `click: stanley-panel cpu`. Popups collect their own detail
data, only while open.

### Known gaps

- `agent-quota` (yadm, `~/.config/systemd/user`) fails its Codex half on
  fractal with `FileNotFoundError`, likely `codex` missing from systemd's
  PATH; its quota is what the agents popup shows.
- The audio device table (`src/widgets/audio.rs`, `DEVICES`) is code. A new
  headset needs a line there, as it did in slstatus.
