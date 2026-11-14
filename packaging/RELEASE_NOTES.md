First public pre-alpha of Fudge, a fast, private keyboard launcher.

## Install (Linux x86_64)

Debian, Ubuntu and derivatives (Ubuntu 24.04 or newer, GTK 4.10+ and libadwaita 1.5+):

```sh
sudo apt install ./fudge_*_amd64.deb
fudge daemon
```

Any other distribution: unpack the tarball and run `./install.sh` (installs to `~/.local`), then `fudge daemon`.

Press Alt+Space to open it. Run `fudge install-autostart` to start it at login.

## What works

Apps, files, math, units, currency, time zones, emoji, colours, kill, system commands, window switching and web searches, on Linux X11. Tested on GNOME.

## Known limits

- Wayland has no global hotkey yet: bind `fudge toggle` to a key in your desktop settings. Window switching on GNOME and KDE Wayland is not done.
- macOS is not packaged and the macOS code is untested.
- Idle memory is about 215 MB, far above the target. This is being worked on.
- File content search and plugins are not implemented yet.

Verify downloads with `sha256sum -c SHA256SUMS`.
