//! Unix-socket control channel: `fudge show|hide|toggle|reindex` talk to the daemon.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    Show,
    Hide,
    Toggle,
    Reindex,
}

impl Cmd {
    pub fn parse(s: &str) -> Option<Cmd> {
        match s.trim() {
            "show" => Some(Cmd::Show),
            "hide" => Some(Cmd::Hide),
            "toggle" => Some(Cmd::Toggle),
            "reindex" => Some(Cmd::Reindex),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Cmd::Show => "show",
            Cmd::Hide => "hide",
            Cmd::Toggle => "toggle",
            Cmd::Reindex => "reindex",
        }
    }
}

pub fn socket_path() -> PathBuf {
    if let Some(d) = std::env::var_os("XDG_RUNTIME_DIR") {
        if cfg!(target_os = "linux") {
            return PathBuf::from(d).join("fudge.sock");
        }
    }
    let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
    std::env::temp_dir().join(format!("fudge-{user}.sock"))
}

/// Send a command to a running daemon. Err if none is listening.
pub fn send(cmd: Cmd) -> std::io::Result<()> {
    let mut s = UnixStream::connect(socket_path())?;
    s.set_read_timeout(Some(Duration::from_secs(2)))?;
    s.set_write_timeout(Some(Duration::from_secs(2)))?;
    writeln!(s, "{}", cmd.as_str())?;
    let mut line = String::new();
    BufReader::new(s).read_line(&mut line)?;
    Ok(())
}

/// Bind the socket (removing a stale one). Err(AddrInUse) if a live daemon owns it.
pub fn bind() -> std::io::Result<UnixListener> {
    let path = socket_path();
    if path.exists() {
        if UnixStream::connect(&path).is_ok() {
            return Err(std::io::Error::new(std::io::ErrorKind::AddrInUse, "daemon already running"));
        }
        let _ = std::fs::remove_file(&path);
    }
    let l = UnixListener::bind(&path)?;
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    Ok(l)
}

/// Accept loop on its own thread; `handle` must be quick and thread-safe.
pub fn serve(listener: UnixListener, handle: impl Fn(Cmd) + Send + 'static) {
    std::thread::Builder::new()
        .name("fudge-ipc".into())
        .spawn(move || {
            for conn in listener.incoming() {
                let Ok(mut conn) = conn else { continue };
                let _ = conn.set_read_timeout(Some(Duration::from_secs(2)));
                let mut line = String::new();
                if BufReader::new(&conn).read_line(&mut line).is_err() {
                    continue;
                }
                match Cmd::parse(&line) {
                    Some(c) => {
                        handle(c);
                        let _ = writeln!(conn, "ok");
                    }
                    None => {
                        let _ = writeln!(conn, "error: unknown command");
                    }
                }
            }
        })
        .expect("spawn ipc thread");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_cmds() {
        assert_eq!(Cmd::parse("toggle\n"), Some(Cmd::Toggle));
        assert_eq!(Cmd::parse(" show "), Some(Cmd::Show));
        assert_eq!(Cmd::parse("nope"), None);
        for c in [Cmd::Show, Cmd::Hide, Cmd::Toggle, Cmd::Reindex] {
            assert_eq!(Cmd::parse(c.as_str()), Some(c));
        }
    }
}
