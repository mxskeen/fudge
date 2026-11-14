//! `kill <name>` and `kill port <n>`.

use std::collections::HashMap;
use std::sync::Mutex;

use fudge_core::fuzzy;
use fudge_core::{Action, Icon, Provider, Query, ResultItem};
use sysinfo::{ProcessesToUpdate, System};

const PROTECTED_EXACT: &[&str] = &[
    "systemd", "init", "xorg", "xwayland", "gnome-shell", "plasmashell", "sshd", "dbus-daemon",
    "dbus-broker", "pipewire", "wireplumber", "pulseaudio", "windowserver", "launchd", "kernel_task",
    "loginwindow", "finder", "dock", "fudge", "gdm", "sddm", "lightdm", "login", "polkitd",
    "networkmanager", "hyprland", "sway",
];

/// Processes whose death would take the session or the machine down with them.
pub fn is_protected(name: &str) -> bool {
    let n = name.to_lowercase();
    PROTECTED_EXACT.contains(&n.as_str())
        || n.starts_with("kwin")
        || n.starts_with("systemd")
        || n.starts_with("gnome-session")
        || n.starts_with("fudge")
}

/// Parse `/proc/net/tcp` or `/proc/net/tcp6` and return socket inodes of
/// LISTEN (state 0A) sockets bound to `port`.
pub fn parse_proc_net_tcp(content: &str, port: u16) -> Vec<u64> {
    content
        .lines()
        .skip(1)
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 10 || f[3] != "0A" {
                return None;
            }
            let local_port = u16::from_str_radix(f[1].rsplit(':').next()?, 16).ok()?;
            if local_port != port {
                return None;
            }
            f[9].parse::<u64>().ok().filter(|i| *i != 0)
        })
        .collect()
}

/// Parse a `kill` query remainder: "port 3000" -> Some(3000).
pub fn parse_port_query(rest: &str) -> Option<Option<u16>> {
    let mut w = rest.split_whitespace();
    if !w.next()?.eq_ignore_ascii_case("port") {
        return None;
    }
    Some(w.next().and_then(|p| p.trim_start_matches(':').parse::<u16>().ok()).filter(|p| *p != 0))
}

#[cfg(target_os = "linux")]
fn pids_listening_on(port: u16) -> Vec<u32> {
    let mut inodes: Vec<u64> = Vec::new();
    for f in ["/proc/net/tcp", "/proc/net/tcp6"] {
        if let Ok(c) = std::fs::read_to_string(f) {
            inodes.extend(parse_proc_net_tcp(&c, port));
        }
    }
    if inodes.is_empty() {
        return Vec::new();
    }
    let targets: Vec<String> = inodes.iter().map(|i| format!("socket:[{i}]")).collect();
    let mut pids = Vec::new();
    let Ok(rd) = std::fs::read_dir("/proc") else { return pids };
    for e in rd.flatten() {
        let Some(pid) = e.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else { continue };
        let Ok(fds) = std::fs::read_dir(e.path().join("fd")) else { continue };
        for fd in fds.flatten() {
            if let Ok(l) = std::fs::read_link(fd.path()) {
                if targets.iter().any(|t| l.as_os_str() == t.as_str()) {
                    pids.push(pid);
                    break;
                }
            }
        }
    }
    pids.sort_unstable();
    pids.dedup();
    pids
}

#[cfg(target_os = "macos")]
fn pids_listening_on(port: u16) -> Vec<u32> {
    let out = std::process::Command::new("lsof")
        .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-t"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    let mut pids: Vec<u32> = out
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().filter_map(|l| l.trim().parse().ok()).collect())
        .unwrap_or_default();
    pids.sort_unstable();
    pids.dedup();
    pids
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn pids_listening_on(_port: u16) -> Vec<u32> {
    Vec::new()
}

fn human_mem(bytes: u64) -> String {
    let mb = bytes as f64 / (1024.0 * 1024.0);
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{:.0} MB", mb)
    }
}

pub struct KillProvider {
    sys: Mutex<System>,
    keywords: Vec<&'static str>,
}

impl KillProvider {
    pub fn new() -> Self {
        Self { sys: Mutex::new(System::new()), keywords: vec!["kill"] }
    }

    fn refresh_processes(&self) -> HashMap<String, (Vec<u32>, u64)> {
        let mut sys = self.sys.lock().unwrap();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let me = std::process::id();
        let mut groups: HashMap<String, (Vec<u32>, u64)> = HashMap::new();
        for (pid, p) in sys.processes() {
            let pid = pid.as_u32();
            if pid == me {
                continue;
            }
            let name = p.name().to_string_lossy().into_owned();
            if name.is_empty() {
                continue;
            }
            let g = groups.entry(name).or_default();
            g.0.push(pid);
            g.1 += p.memory();
        }
        groups
    }

    fn by_name(&self, text: &str) -> Vec<ResultItem> {
        let groups = self.refresh_processes();
        let mut hits: Vec<(f32, String, Vec<u32>, u64)> = groups
            .into_iter()
            .filter_map(|(name, (mut pids, mem))| {
                let s = fuzzy::score(text, &name)?;
                if s < 45.0 {
                    return None;
                }
                pids.sort_unstable();
                Some((s, name, pids, mem))
            })
            .collect();
        hits.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then(b.3.cmp(&a.3)));
        hits.truncate(8);
        hits.into_iter()
            .map(|(s, name, pids, mem)| {
                let n = pids.len();
                let protected = is_protected(&name);
                let mut sub = format!("{n} process{} · {}", if n == 1 { "" } else { "es" }, human_mem(mem));
                if protected {
                    sub.push_str(" · system process, asks first");
                }
                ResultItem::new(format!("kill:{name}"), format!("Kill {name}"), "kill")
                    .subtitle(sub)
                    .icon(Icon::Named("process-stop".into()))
                    .score(s)
                    .action(Action::KillProcesses { pids, name, protected })
                    .no_history()
            })
            .collect()
    }

    fn by_port(&self, port: u16) -> Vec<ResultItem> {
        let pids = pids_listening_on(port);
        if pids.is_empty() {
            return Vec::new();
        }
        let mut sys = self.sys.lock().unwrap();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let mut names: Vec<String> = pids
            .iter()
            .filter_map(|p| sys.process(sysinfo::Pid::from_u32(*p)).map(|pr| pr.name().to_string_lossy().into_owned()))
            .collect();
        names.sort();
        names.dedup();
        let protected = names.iter().any(|n| is_protected(n));
        let label = if names.is_empty() { "unknown process".to_string() } else { names.join(", ") };
        let pid_list = pids.iter().map(u32::to_string).collect::<Vec<_>>().join(", ");
        vec![ResultItem::new(format!("kill:port:{port}"), format!("Kill {label} on port {port}"), "kill")
            .subtitle(format!("pid {pid_list}{}", if protected { " · system process, asks first" } else { "" }))
            .icon(Icon::Named("process-stop".into()))
            .score(100.0)
            .action(Action::KillProcesses { pids, name: format!("{label} (port {port})"), protected })
            .no_history()]
    }
}

impl Default for KillProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for KillProvider {
    fn id(&self) -> &'static str {
        "kill"
    }

    fn keywords(&self) -> &[&'static str] {
        &self.keywords
    }

    fn query(&self, q: &Query) -> Vec<ResultItem> {
        if q.keyword.as_deref() != Some("kill") || q.rest.is_empty() {
            return Vec::new();
        }
        match parse_port_query(&q.rest) {
            Some(Some(port)) => self.by_port(port),
            Some(None) => Vec::new(), // "kill port" with no number yet
            None => self.by_name(&q.rest),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TCP4: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 0100007F:0BB8 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 123456 1 0000000000000000 100 0 0 10 0
   1: 00000000:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 222222 1 0000000000000000 100 0 0 10 0
   2: 0100007F:0BB8 0100007F:D2F4 01 00000000:00000000 00:00000000 00000000  1000        0 333333 1 0000000000000000 100 0 0 10 0
";
    const TCP6: &str = "  sl  local_address                         remote_address                        st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 00000000000000000000000000000000:0BB8 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 987654 1 0000000000000000 100 0 0 10 0
";

    #[test]
    fn parses_listening_sockets_only() {
        assert_eq!(parse_proc_net_tcp(TCP4, 3000), vec![123456]);
        assert_eq!(parse_proc_net_tcp(TCP4, 8080), vec![222222]);
        assert!(parse_proc_net_tcp(TCP4, 80).is_empty());
    }

    #[test]
    fn parses_ipv6() {
        assert_eq!(parse_proc_net_tcp(TCP6, 3000), vec![987654]);
    }

    #[test]
    fn tolerates_garbage() {
        assert!(parse_proc_net_tcp("", 80).is_empty());
        assert!(parse_proc_net_tcp("header\nnot a real line", 80).is_empty());
    }

    #[test]
    fn port_query_parsing() {
        assert_eq!(parse_port_query("port 3000"), Some(Some(3000)));
        assert_eq!(parse_port_query("PORT :8080"), Some(Some(8080)));
        assert_eq!(parse_port_query("port"), Some(None));
        assert_eq!(parse_port_query("port abc"), Some(None));
        assert_eq!(parse_port_query("portal"), None);
        assert_eq!(parse_port_query("spotify"), None);
    }

    #[test]
    fn protected_names() {
        for n in ["systemd", "Xorg", "Xwayland", "gnome-shell", "kwin_wayland", "kwin_x11", "plasmashell", "sshd", "WindowServer", "launchd", "Finder", "Dock", "fudge", "pipewire"] {
            assert!(is_protected(n), "{n}");
        }
        assert!(!is_protected("spotify"));
        assert!(!is_protected("firefox"));
    }

    #[test]
    fn memory_format() {
        assert_eq!(human_mem(412 * 1024 * 1024), "412 MB");
        assert_eq!(human_mem(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn requires_keyword() {
        let p = KillProvider::new();
        assert!(p.query(&Query::parse("spotify", |_| false)).is_empty());
        assert!(p.query(&Query::parse("kill", |k| k == "kill")).is_empty());
    }
}
