# Fudge Launcher — open-source keyboard launcher for Linux & macOS

## Context
Fudge is a keyboard launcher: Alt+Space opens a box in ~10 ms (~55 MB RAM), you type, you get the answer — apps, files, math/units/currency, time zones, emoji, system commands, window switching, web searches on request, plugins, clipboard history, snippets, and a strict "nothing you type leaves your machine" privacy stance. Plus a project website (in-browser live demo, run log, privacy counter, changelog, install help).

**Fudge Launcher** is an open-source (MIT/Apache-2.0) equivalent for Linux distros and macOS, end to end: app + website. Decisions taken with the user:
- Stack: **Rust + GTK4/libadwaita** (native, low memory), with thin per-OS platform glue.
- v1 = **core launcher MVP**; clipboard history, snippets, connectors, recording/Studio are post-v1 milestones.
- **Website included** (no pricing/checkout/licence — it's free).
- **Open plugin API** (community plugins + git-backed registry), not first-party only.

No licence server, no activation, no account. Network calls in v1: currency rates (hourly, toggleable) and an opt-in update check. That's it.

## Feature parity map (v1 MVP)
| Query | Fudge v1 | Linux impl | macOS impl |
|---|---|---|---|
| Alt+Space anywhere | yes (configurable) | X11: XGrabKey (x11rb). Wayland: XDG GlobalShortcuts portal (KDE, Hyprland, GNOME 48+); fallback = `fudge toggle` bound as DE custom shortcut (installer offers to add it on GNOME via gsettings) | Carbon `RegisterEventHotKey` (Alt+Space; Cmd+Space left to Spotlight) |
| `spo` → app | yes, fuzzy + frecency | XDG `.desktop` files (system, `~/.local`, Flatpak & Snap exports), `gio launch` | `.app` bundles in /Applications, ~/Applications, /System/Applications; `open -a` |
| `budget` → file by name | yes | own index: walk + inotify (`notify` crate) over home folders, stored in SQLite/fst; optional "every drive" via plocate DB | `mdfind` (Spotlight) + same own index for name search |
| file contents | yes, if OS indexer present | GNOME LocalSearch/Tracker3 over D-Bus, Baloo on KDE; hidden otherwise | Spotlight `kMDItemTextContent` |
| `12 * 18`, `72 f to c` | yes | own expression parser + unit table (pure Rust, shared with website via WASM) | same |
| `12 usd to inr` | yes | open.er-api.com, ≤1/hour, in-memory, "Network features" switch | same |
| `time tokyo` | yes | `chrono-tz` + city→tz table | same |
| `emoji shrug` | yes, copies | `emojis` crate | same |
| `#3b82f6` colour | yes | preview swatch, rgb/hsl | same |
| `kill spotify`, `kill port 3000` | yes, confirm on protected procs | `sysinfo` + `/proc/net/tcp*` | `sysinfo` + `libproc`/`lsof` |
| `lock`, `sleep`, `restart`, `shutdown`, `logout` | yes, confirm destructive | `loginctl`/logind D-Bus | `pmset`, AppleScript `System Events` |
| `win figma` | yes | X11 EWMH; KDE via KWin script D-Bus; Hyprland `hyprctl`; Sway `swaymsg`; GNOME via small bundled Shell extension exposing D-Bus | `CGWindowListCopyWindowInfo` + `NSRunningApplication.activate` + AX raise (Accessibility permission) |
| `set bluetooth` (settings) | yes | `gnome-control-center <panel>` / `systemsettings <kcm>` table | `x-apple.systempreferences:` URLs |
| `/online q`, `/yt`, `/gh`… | yes, never automatic | `xdg-open` | `open` |
| `> cmd` shell | yes, audit-logged | `$SHELL -c` in a terminal or detached | same |
| `folders` quick folders | yes | XDG user dirs + recents | `~` well-known dirs |
| History/Recents | yes, local, window switches never stored | SQLite in `$XDG_DATA_HOME/fudge` | `~/Library/Application Support/Fudge` |
| Styles | Solid (default) + Frosted | compositor blur (KDE blur protocol, Hyprland layerrule); never screen-capture | `NSVisualEffectView` |

Post-v1 milestones: clipboard history (encrypted, password-manager skip list, pins, transforms), snippets (`;sig`, `{date}/{cursor}`, Raycast import), connectors (`/sp` via MPRIS/AppleScript, `/ob`, `/vs`, `/steam`, `/nt`), plugin registry UI, screen recording + editor.

## Architecture
Resident daemon with the window pre-built and hidden; hotkey just shows it and focuses the entry → target <15 ms open, <60 MB RSS.

Cargo workspace:
```
fudge/
  crates/
    fudge-core/       Query, Result, Action, Provider trait, ranking (nucleo fuzzy + frecency), history (rusqlite), config (TOML, serde)
    fudge-compute/    calc, units, currency, time, emoji, colour — no I/O except injectable rate fetcher; compiles to wasm32
    fudge-providers/  apps, files, windows, system cmds, settings panels, web search, shell, quick folders
    fudge-platform/   trait Platform { hotkey, list_windows, focus_window, lock, sleep, paste, open } with linux/{x11,wayland,gnome,kde,hyprland,sway} and macos/ backends
    fudge-index/      file indexer + `fudge-indexd` (separate process, inotify/FSEvents, so UI stays light)
    fudge-plugin/     plugin host: subprocess JSON-RPC over stdio, manifest parsing, timeouts, sandbox hints
  apps/
    fudge/            GTK4/libadwaita UI + daemon + CLI (`fudge`, `fudge toggle`, `fudge query <q> --json`, `fudge reindex`); single-instance via Unix socket
    fudge-settings/   libadwaita preferences window (hotkey, style, search scope, network features, history, plugins)
  extensions/gnome-shell/  tiny extension for window list/focus on GNOME Wayland
  plugins/examples/  Python + shell example plugins
  web/               Astro static site
  packaging/         flatpak? (no — see below), deb, rpm, AUR PKGBUILD, AppImage, Nix flake, Homebrew cask, .dmg script
  docs/
```
- **Provider trait**: `fn matches(&self, q:&Query) -> Vec<Result>` async with per-provider deadline (e.g. 30 ms); prefix keywords (`kill`, `win`, `time`, `/`, `>`) route directly; results streamed into the list as they arrive.
- **UI**: single undecorated window; on wlroots/KDE use `gtk4-layer-shell` (overlay, centered, keyboard exclusive); GNOME uses a normal toplevel. Keys: ↑↓ move, Enter run, Esc clear/hide, Tab complete, Ctrl+Enter alt action, Enter copies computed answers.
- **Plugins**: `~/.local/share/fudge/plugins/<id>/plugin.toml` (id, keyword, exec, version). Host spawns on first use, keeps alive, sends `{"method":"query","params":{"text":…}}`, gets results with actions (`open`, `copy`, `run`, `paste`). Built-ins implement the same trait, so external plugins are first-class. Registry = a GitHub repo with `index.json` signed via minisign; app verifies before install.
- **Privacy**: no telemetry, no crash upload; `PRIVACY.md` lists every network call (every call, with what is sent). Bug report = locally built text the user pastes into a GitHub issue.
- **Distribution**: AppImage, .deb, .rpm, AUR, Nix, and a `curl | sh`-free install doc; macOS universal `.dmg` + Homebrew cask (ad-hoc signed; install page explains Gatekeeper "Open anyway"). Flatpak deferred — its sandbox blocks process kill/window control/app launching without heavy `flatpak-spawn --host` workarounds. Updates: distro package managers; opt-in "check GitHub releases" notifier; SHA-256 + minisign signatures on every release.

## Website (web/, Astro, deployed to GitHub Pages / Cloudflare Pages)
Own brand (name "fudge", own palette/logo). Sections:
1. Hero: "Press Alt Space. Type. Get the answer." + **live in-browser launcher** powered by `fudge-compute` compiled to WASM (real calc/units/time/emoji; currency fetched from the same API; app/file/kill rows simulated).
2. Stats: open time, memory, tracking 0, price $0 / open source — measured numbers only, with footnotes.
3. Reel: scroll-scrubbed recording of the real app (12 beats, recorded on GNOME + macOS).
4. Run log: query → result table with "Try ↵" driving the demo.
5. Honest comparison: Ulauncher, Albert, KRunner, Rofi, GNOME Search / Spotlight, Raycast, Alfred.
6. Privacy: live counter of "queries sent anywhere: 0", list of on-device vs. network calls.
7. Plugins: browse registry, "write a plugin in 20 lines".
8. FAQ, then pages: /download (per-distro + mac, checksums), /install (hotkey setup per DE, Gatekeeper), /changelog (generated from CHANGELOG.md), /privacy, /plugins, /press.

## Milestones
- **M0** Workspace, CI (GitHub Actions: Linux + macOS build/test/clippy), LICENSE, CONTRIBUTING, PRIVACY.md.
- **M1** Daemon + GTK window + hotkey (X11, portal, fallback CLI; macOS Carbon) + apps provider + frecency. Measure open latency.
- **M2** `fudge-compute` (calc, units, currency, time, emoji, colour) with golden tests from the run log.
- **M3** Files (own indexer + Spotlight/Tracker contents), quick folders, web search, shell.
- **M4** System commands, kill/kill port, settings panels, window switching across X11/KDE/Hyprland/Sway/GNOME-extension/macOS.
- **M5** Settings app, styles, plugin host + example plugins + signed registry.
- **M6** Packaging + website + v1.0.0 release.
- Post-v1: clipboard, snippets, connectors, recording.

## Verification
- `cargo test --workspace`: golden tests for each run-log query (`12 * 18`→216, `72 f to c`→22.222222 °C, `time tokyo` format, `emoji shrug`→🤷, currency with mocked rates, kill-port parsing fixtures).
- `fudge query "<q>" --json` CLI for scripted end-to-end checks of provider output on each platform.
- Latency: daemon logs hotkey-received → first frame presented (GTK frame clock); assert p50 < 15 ms. Memory: read RSS from `/proc/self/status` / `task_info` at idle; target < 60 MB.
- Manual matrix per release: GNOME Wayland, KDE Wayland, Hyprland, Sway, XFCE/i3 on X11, macOS 14/15 (Apple Silicon + Intel).
- Network audit: run under `strace -e connect` / Little Snitch with network features off → zero connections.
- Website: `npm run build`, Lighthouse, demo answers match `fudge-compute` tests (same crate).
