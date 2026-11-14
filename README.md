# fudge

**Press Alt+Space. Type. Get the answer.**

Fudge is a fast, private, open-source keyboard launcher for Linux and macOS. One box for apps, files, math, units, currency, time zones, emoji, colours, system commands, window switching and web searches you ask for.

- **Fast.** It's a native Rust + GTK4 app that stays running with its window already built, so Alt+Space only has to show it.
- **Private.** No account, no telemetry. Nothing you type leaves your machine. See [PRIVACY.md](PRIVACY.md).
- **Free.** MIT or Apache-2.0, your choice. Plugins can be written in any language.

## What you can type

| You type | You get |
|---|---|
| `fire` | Firefox · App |
| `budget` | budget-2026.ods · file |
| `12 * 18` | = 216 |
| `72 f to c` | = 22.222222 °C |
| `12 usd to inr` | INR at today's rate (the only online answer) |
| `time tokyo` | Tokyo's time, day and UTC offset |
| `emoji shrug` | 🤷 copied |
| `#3b82f6` | colour swatch, rgb() and hsl() |
| `kill spotify` | ends Spotify and its helpers |
| `kill port 3000` | ends whatever listens on :3000 |
| `lock` / `sleep` / `restart` | does that (asks first when it can't be undone) |
| `win code` | brings the VS Code window forward |
| `/online rust lifetimes` | opens your browser; never automatic |
| `> htop` | runs a shell command |

## Status

Pre-alpha. Roadmap in [docs/PLAN.md](docs/PLAN.md).

## Building

```sh
# Debian/Ubuntu
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev \
  libgtk4-layer-shell-dev libsqlite3-dev libx11-dev libxcb1-dev
cargo build --release
./target/release/fudge          # start the daemon
./target/release/fudge toggle   # show/hide (bind this to a key on Wayland)
./target/release/fudge query "72 f to c" --json
```

macOS: `brew install gtk4 libadwaita pkg-config`, then `cargo build --release`.
