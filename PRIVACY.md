# Privacy

Fudge runs on your machine. Searches, math, unit conversion, time zones, file search, system commands and history never leave it. There is no account, no telemetry, no crash upload and no analytics.

## Every network call Fudge can make

| Call | When | Sent | Off switch |
|---|---|---|---|
| Currency rates | You type a currency conversion, at most once an hour | Nothing about you: a plain request for today's USD rates to `open.er-api.com`, the same for every user. The amount and currencies stay on your machine. | `network_features = false` in config, or Settings → Privacy |
| Update check | Only if you turn it on | Nothing: a plain request for the latest GitHub release | Off by default |
| Plugin registry | Only when you browse or install plugins | Nothing beyond the download request | Don't open the plugin browser |

Web searches you start with `/online`, `/yt`, `/gh` and so on open in your browser. Fudge never falls back to the web by itself.

Like any web request, these reveal your IP address to the server that answers them.

## What stays on your machine

- **History:** the results you run (title, what it does, how often, when), used to rank them. Stored in `$XDG_DATA_HOME/fudge/history.sqlite` (Linux) or `~/Library/Application Support/Fudge/` (macOS). Window switches and web searches are never stored. You can turn history off and clear it.
- **File index:** file names, sizes and dates for fast search. Never file contents. Content search goes through your OS's own indexer (Tracker/LocalSearch, Baloo, or Spotlight).
- **Audit log:** each `>` shell command and each confirmed irreversible action (such as ending a process).

Delete the folders above to remove everything.
