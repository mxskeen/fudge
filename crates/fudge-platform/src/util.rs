use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

/// Spawn a child fully detached: null stdio, own process group, reaped in a
/// background thread so the launcher never blocks and never leaves zombies.
pub(crate) fn spawn_detached(cmd: &mut Command) -> Result<()> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to start {:?}", cmd.get_program()))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

pub(crate) fn spawn_args(program: &str, args: &[&str]) -> Result<()> {
    spawn_detached(Command::new(program).args(args))
}

/// Try each command line in turn; the first whose program exists wins.
#[allow(dead_code)]
pub(crate) fn spawn_first(candidates: &[&[&str]]) -> Result<()> {
    let mut last = None;
    for c in candidates {
        let Some((prog, args)) = c.split_first() else { continue };
        if find_in_path(prog).is_none() {
            continue;
        }
        match spawn_args(prog, args) {
            Ok(()) => return Ok(()),
            Err(e) => last = Some(e),
        }
    }
    match last {
        Some(e) => Err(e),
        None => anyhow::bail!(
            "none of these programs is installed: {}",
            candidates
                .iter()
                .filter_map(|c| c.first())
                .copied()
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// Run to completion and capture stdout (for quick CLI queries like hyprctl).
#[allow(dead_code)]
pub(crate) fn run_capture(program: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .with_context(|| format!("failed to run {program}"))?;
    if !out.status.success() {
        anyhow::bail!("{program} exited with {}", out.status);
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn find_in_path(bin: &str) -> Option<PathBuf> {
    if bin.contains('/') {
        let p = PathBuf::from(bin);
        return p.is_file().then_some(p);
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join(bin))
        .find(|p| p.is_file())
}

const FIELD_CODES: &str = "fFuUdDnNickvm";

/// Split a desktop-entry `Exec=` value into argv: honours double quotes and
/// backslash escapes, drops field codes (%f %F %u %U %i %c %k ...), and turns
/// `%%` into `%`.
pub fn exec_to_argv(exec: &str) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_arg = false;
    let mut in_quote = false;
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_quote = !in_quote;
                in_arg = true;
            }
            '\\' if in_quote => {
                if let Some(n) = chars.next() {
                    cur.push(n);
                    in_arg = true;
                }
            }
            c if c.is_whitespace() && !in_quote => {
                if in_arg {
                    args.push(std::mem::take(&mut cur));
                    in_arg = false;
                }
            }
            '%' => {
                match chars.peek().copied() {
                    Some('%') => {
                        chars.next();
                        cur.push('%');
                        in_arg = true;
                    }
                    Some(n) if FIELD_CODES.contains(n) => {
                        chars.next();
                        // A bare code is dropped entirely; `in_arg` stays as is so
                        // "--x=%f" keeps "--x=" and a lone "%f" vanishes.
                    }
                    _ => {
                        cur.push('%');
                        in_arg = true;
                    }
                }
            }
            c => {
                cur.push(c);
                in_arg = true;
            }
        }
    }
    if in_arg {
        args.push(cur);
    }
    args.retain(|a| !a.is_empty());
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_field_codes() {
        assert_eq!(exec_to_argv("firefox %u"), vec!["firefox"]);
        assert_eq!(
            exec_to_argv("env FOO=1 /usr/bin/code --unity-launch %F"),
            vec!["env", "FOO=1", "/usr/bin/code", "--unity-launch"]
        );
        assert_eq!(exec_to_argv("app %i %c %k"), vec!["app"]);
    }

    #[test]
    fn quotes_and_percent() {
        assert_eq!(
            exec_to_argv(r#""/opt/My App/run" --name "a b" 100%%"#),
            vec!["/opt/My App/run", "--name", "a b", "100%"]
        );
    }
}
