//! Fudge Launcher: GTK4 front end. One resident daemon; everything else talks to it over a socket.

mod hotkey;
mod install;
mod ipc;
mod keys;
#[cfg(target_os = "linux")]
mod placement;
mod ui;

use std::sync::Arc;

use adw::prelude::*;
use anyhow::Result;
use clap::{Parser, Subcommand};
use fudge_core::Config;
use gtk4 as gtk;
use libadwaita as adw;

use ipc::Cmd;

#[derive(Parser)]
#[command(name = "fudge", version, about = "Fudge Launcher: open-source Alt+Space launcher")]
struct Cli {
    #[command(subcommand)]
    command: Option<Sub>,
}

#[derive(Subcommand)]
enum Sub {
    /// Start the resident daemon (default). If one is running, ask it to show and exit.
    Daemon,
    /// Toggle the launcher window of the running daemon.
    Toggle,
    /// Show the launcher window.
    Show,
    /// Hide the launcher window.
    Hide,
    /// Print engine results for a query without opening the GUI.
    Query {
        query: String,
        #[arg(long)]
        json: bool,
    },
    /// Ask the daemon to rescan apps and files (or rescan briefly in-process if none runs).
    Reindex,
    /// Print the config file path.
    ConfigPath,
    /// Write ~/.config/autostart/fudge.desktop so the daemon starts at login.
    InstallAutostart,
    /// GNOME: register the configured hotkey as a custom keyboard shortcut running `fudge toggle`.
    InstallShortcut,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    let res = match cli.command.unwrap_or(Sub::Daemon) {
        Sub::Daemon => daemon(),
        Sub::Toggle => remote(Cmd::Toggle),
        Sub::Show => remote(Cmd::Show),
        Sub::Hide => remote(Cmd::Hide),
        Sub::Query { query, json } => query_cmd(&query, json),
        Sub::Reindex => reindex(),
        Sub::ConfigPath => {
            match Config::path() {
                Some(p) => println!("{}", p.display()),
                None => eprintln!("no config directory on this system"),
            }
            Ok(())
        }
        Sub::InstallAutostart => install::install_autostart(),
        Sub::InstallShortcut => install::install_shortcut(&Config::load().hotkey),
    };
    if let Err(e) = res {
        eprintln!("fudge: {e:#}");
        std::process::exit(1);
    }
}

fn remote(cmd: Cmd) -> Result<()> {
    ipc::send(cmd).map_err(|e| {
        anyhow::anyhow!("cannot reach the daemon at {} ({e}). Start it with `fudge daemon`.", ipc::socket_path().display())
    })
}

fn reindex() -> Result<()> {
    if ipc::send(Cmd::Reindex).is_ok() {
        println!("Asked the running daemon to reindex.");
        return Ok(());
    }
    let (engine, _p) = fudge_providers::build_engine();
    engine.refresh_all();
    std::thread::sleep(std::time::Duration::from_secs(3));
    println!("Reindexed (no daemon was running).");
    Ok(())
}

fn query_cmd(q: &str, json: bool) -> Result<()> {
    let (engine, _p) = fudge_providers::build_engine();
    // Let background scans (apps, file index) produce something first.
    std::thread::sleep(std::time::Duration::from_millis(1500));
    let r = engine.search(q, engine.config().max_results);
    if json {
        println!("{}", serde_json::to_string_pretty(&r)?);
    } else if r.is_empty() {
        println!("(no results)");
    } else {
        for (i, it) in r.iter().enumerate() {
            let act = it.actions.first().map(|a| a.label()).unwrap_or("-");
            println!("{}. {}  [{}] ({act})", i + 1, it.title, it.provider);
            if !it.subtitle.is_empty() {
                println!("   {}", it.subtitle);
            }
        }
    }
    Ok(())
}

fn daemon() -> Result<()> {
    let listener = match ipc::bind() {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            let _ = ipc::send(Cmd::Show);
            eprintln!("fudge: a daemon is already running; asked it to show.");
            return Ok(());
        }
        Err(e) => anyhow::bail!("cannot bind {}: {e}", ipc::socket_path().display()),
    };
    let _cleanup = SocketCleanup;
    let (engine, platform) = fudge_providers::build_engine();
    let engine = Arc::new(engine);
    let hotkey_spec = engine.config().hotkey.clone();

    ipc::serve(listener, |cmd| ui::dispatch(cmd, 0));

    let app = adw::Application::builder()
        .application_id("org.fudge.Launcher")
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.connect_activate(move |app| {
        ui::build(app, engine.clone(), platform.clone());
        // `Alt+space` etc. Failure is logged with instructions, never fatal.
        hotkey::start(&hotkey_spec, |time| ui::dispatch(Cmd::Toggle, time));
        if ui::debug_timing() {
            gtk::glib::timeout_add_seconds_local_once(5, || {
                let rss = std::fs::read_to_string("/proc/self/status")
                    .ok()
                    .and_then(|s| s.lines().find(|l| l.starts_with("VmRSS:")).map(str::to_string));
                eprintln!("[timing] idle {}", rss.unwrap_or_else(|| "VmRSS: unavailable".into()));
            });
        }
    });
    // Keep running with the window hidden.
    std::mem::forget(app.hold());
    app.run_with_args::<&str>(&[]);
    Ok(())
}

struct SocketCleanup;
impl Drop for SocketCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(ipc::socket_path());
    }
}
