//! stanley: status bar content for dwl. See docs/design.md.

mod click;
mod config;
mod meter;
mod protocol;
mod render;
mod blocks;

use config::{Config, Entry};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use blocks::{Reading, Block};

static STOP: AtomicBool = AtomicBool::new(false);
static RELOAD: AtomicBool = AtomicBool::new(false);
static WAKE: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(signal: libc::c_int) {
    match signal {
        libc::SIGHUP => RELOAD.store(true, Ordering::SeqCst),
        libc::SIGUSR1 => WAKE.store(true, Ordering::SeqCst),
        _ => STOP.store(true, Ordering::SeqCst),
    }
}

/// SIGTERM and SIGINT exit 0. SIGHUP re-reads the bar file; SIGUSR1 redraws
/// now. SIGPIPE keeps its default: dwl-session tells that dwl has gone by our
/// dying of it (exit 141), and Rust would otherwise ignore it.
fn install_signals() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = on_signal as extern "C" fn(libc::c_int) as usize;
        libc::sigemptyset(&mut action.sa_mask);
        // No SA_RESTART: a signal has to cut the wait short.
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGUSR1] {
            libc::sigaction(signal, &action, std::ptr::null_mut());
        }
    }
}

struct Options {
    once: bool,
    check: bool,
    path: Option<PathBuf>,
}

fn options() -> Options {
    let mut options = Options { once: false, check: false, path: None };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-1" => options.once = true,
            "--check" => options.check = true,
            "-c" => options.path = args.next().map(PathBuf::from),
            _ => {
                eprintln!("usage: stanley [-1] [--check] [-c bar-file]");
                std::process::exit(2);
            }
        }
    }
    options
}

/// The bar file, or the built-in bar when there is none. Problems are
/// reported and the rest of the file still applies; the count is returned.
fn load(options: &Options) -> (Config, usize) {
    let path = options.path.clone().or_else(config::default_path);
    let (text, name) = match path.as_ref().map(std::fs::read_to_string) {
        Some(Ok(text)) => (text, path.as_ref().unwrap().display().to_string()),
        result => {
            if options.path.is_some() {
                eprintln!("stanley: {}: {}", path.as_ref().unwrap().display(), result.unwrap().unwrap_err());
                std::process::exit(1);
            }
            (config::DEFAULT.to_string(), "built-in bar".to_string())
        }
    };
    let mut problems = 0;
    let mut config = config::parse(&text, &mut |line, message| {
        problems += 1;
        eprintln!("stanley: {name}:{line}: {message}");
    });
    if config.entries.len() > protocol::MAX_BLOCKS {
        eprintln!("stanley: {name}: only the first {} widgets fit the protocol", protocol::MAX_BLOCKS);
        config.entries.truncate(protocol::MAX_BLOCKS);
        problems += 1;
    }
    (config, problems)
}

/// One widget line and its state. Slots stay in bar-file order, unknown
/// widgets included, so a click's block index finds its line.
struct Slot {
    entry: Entry,
    block: Option<Box<dyn Block>>,
    reading: Reading,
    due: Option<Instant>,
    text: String,
    bare: bool,
}

fn build(config: &Config, problems: &mut usize) -> Vec<Slot> {
    let mut factory = blocks::Factory::new();
    config
        .entries
        .iter()
        .map(|entry| {
            let block = factory
                .build(&entry.widget, entry.arg.as_deref())
                .map_err(|e| {
                    *problems += 1;
                    eprintln!("stanley: {e}");
                })
                .ok();
            Slot { entry: entry.clone(), block, reading: Reading::default(), due: None, text: String::new(), bare: false }
        })
        .collect()
}

fn main() {
    let options = options();
    let (mut config, mut problems) = load(&options);
    let mut slots = build(&config, &mut problems);
    if options.check {
        std::process::exit(if problems > 0 { 1 } else { 0 });
    }
    install_signals();
    let mut clicks = if options.once { None } else { click::Clicks::open() };
    let version = protocol::version();
    let stdout = std::io::stdout();

    loop {
        if RELOAD.swap(false, Ordering::SeqCst) {
            let (c, mut p) = load(&options);
            slots = build(&c, &mut p);
            config = c;
        }
        WAKE.store(false, Ordering::SeqCst);
        let settings = config.settings;
        let tick = Duration::from_millis(settings.interval_ms);
        let start = Instant::now();

        for slot in &mut slots {
            let Some(block) = slot.block.as_mut() else { continue };
            // Half a tick of slack keeps a widget due on the fifth tick from
            // slipping to the sixth when that tick lands a little early.
            if slot.due.is_none_or(|due| start + tick / 2 >= due) {
                slot.reading = block.read();
                slot.due = block.interval().map(|every| start + every);
            }
            (slot.text, slot.bare) = render::block(slot.entry.parts, &slot.reading, &settings);
        }
        let placed: Vec<render::Placed> = slots
            .iter()
            .enumerate()
            .map(|(index, slot)| render::Placed {
                index,
                group: slot.entry.group,
                section: slot.entry.section,
                text: &slot.text,
                bare: slot.bare,
            })
            .collect();
        let mut line = render::line(&placed, &settings, version);
        line.push('\n');
        let mut out = stdout.lock();
        if out.write_all(line.as_bytes()).and_then(|_| out.flush()).is_err() {
            std::process::exit(1);
        }
        drop(out);

        if options.once || STOP.load(Ordering::SeqCst) {
            break;
        }
        wait(start + tick, &mut clicks, &mut slots);
        if STOP.load(Ordering::SeqCst) {
            break;
        }
    }
}

/// Sleep until `deadline`, running clicks as they come. A signal or a click
/// ends the wait early: a click may change what its block shows.
fn wait(deadline: Instant, clicks: &mut Option<click::Clicks>, slots: &mut [Slot]) {
    loop {
        if STOP.load(Ordering::SeqCst) || RELOAD.load(Ordering::SeqCst) || WAKE.load(Ordering::SeqCst) {
            return;
        }
        let Some(left) = deadline.checked_duration_since(Instant::now()) else { return };
        let timeout = left.as_millis().clamp(1, i32::MAX as u128) as libc::c_int;
        let Some(c) = clicks.as_mut() else {
            unsafe { libc::poll(std::ptr::null_mut(), 0, timeout) };
            continue;
        };
        let mut fds = c.pollfds();
        let mut clicked = false;
        if unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) } > 0 {
            for click in c.drain() {
                let Some(slot) = (click.block as usize).checked_sub(1).and_then(|i| slots.get_mut(i)) else {
                    continue;
                };
                // The bar file's click: wins; otherwise the block's own.
                let command = match &slot.entry.click {
                    Some(command) => Some(command.clone()),
                    None => slot.block.as_mut().and_then(|b| b.click(click.button)),
                };
                if let Some(command) = command {
                    c.run(&command, click);
                }
                slot.due = None;
                clicked = true;
            }
        }
        c.reap();
        if clicked {
            return;
        }
    }
}
