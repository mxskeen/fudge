# fudge

A fast, private keyboard launcher for Linux and macOS. Press Alt+Space, type, get the answer.

Fudge is free and open source under MIT or Apache-2.0. It has no account, no telemetry and no licence key.

**Status: pre-alpha.** The core works on Linux (X11, tested on GNOME). Wayland, macOS and packaging are not finished. There are no releases yet.

## What it does

| Type | Result |
|---|---|
| `fire` | Launch Firefox |
| `budget` | Find files by name |
| `12 * 18` | `= 216` |
| `72 f to c` | `= 22.222222 °C` |
| `12 usd to inr` | Currency conversion (the only feature that uses the network) |
| `time tokyo` | Time in another city |
| `emoji shrug` | Copy an emoji |
| `#3b82f6` | Colour preview and conversions |
| `kill spotify`, `kill port 3000` | End a process or whatever holds a port |
| `lock`, `sleep`, `restart` | System commands (destructive ones ask first) |
| `win code` | Switch to a window |
| `/yt lofi`, `/gh rust` | Web search, only when you ask |
| `> htop` | Run a shell command |

## Build

Requires Rust 1.80 or newer.

```sh
# Debian / Ubuntu
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev \
  libsqlite3-dev libx11-dev libxcb1-dev

cargo build --release
./target/release/fudge daemon
```

On macOS, install `gtk4`, `libadwaita` and `pkg-config` with Homebrew. The macOS code compiles only on a Mac and has not been tested.

## Use

```sh
fudge daemon              # start the resident launcher
fudge toggle              # show or hide it
fudge query "72 f to c"   # run a query without the UI (add --json for JSON)
fudge install-autostart   # start on login
fudge install-shortcut    # bind Alt+Space on GNOME
```

On X11, Alt+Space is grabbed directly. On Wayland there is no global grab yet: bind `fudge toggle` to a key in your desktop settings.

## Layout

| Path | Purpose |
|---|---|
| `crates/fudge-compute` | Math, units, currency, time zones, emoji, colour. No I/O; also builds to WebAssembly |
| `crates/fudge-core` | Query, result and action types, ranking, history, config |
| `crates/fudge-platform` | Linux and macOS system glue |
| `crates/fudge-providers` | Apps, files, windows, kill, system, web search, shell |
| `apps/fudge` | GTK4 window, daemon and CLI |
| `web` | Project website (Astro) |

Run the tests with `cargo test --workspace`.

## Privacy

Nothing you type leaves your machine. The only network call is the currency rate lookup, at most once an hour, and it can be turned off. Every call is listed in [PRIVACY.md](PRIVACY.md).

## Roadmap

See [docs/PLAN.md](docs/PLAN.md). Planned next: Wayland window switching, a plugin API, clipboard history, snippets and packages.

## Licence

MIT or Apache-2.0, at your option. See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).
