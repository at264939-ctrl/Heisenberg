mod blue_sky;
mod gus;
mod hank;
mod heisenberg;
mod jesse;
mod mike;
mod saul;
mod the_lab;
mod the_rv;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use crate::heisenberg::Orchestrator;
use crate::saul::{Cli, Config};
use serde_json::json;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use tokio::task;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // ── Redirect all tracing to log file (keep terminal pristine) ──
    let log_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/heisenberg");
    std::fs::create_dir_all(&log_dir).ok();
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("heisenberg.log"))
        .unwrap_or_else(|_| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/heisenberg.log")
                .expect("Cannot open any log file")
        });

    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
        )
        .with_writer(std::sync::Mutex::new(log_file))
        .with_target(false)
        .with_ansi(false)
        .compact()
        .init();

    let config = Config::load(cli.config.as_deref())?;

    // Start a background IPC listener (FIFO) for Bash ↔ Rust JSON exchange
    let cmd_fifo = PathBuf::from("/tmp/heisenberg_cmd.fifo");
    let resp_fifo = PathBuf::from("/tmp/heisenberg_resp.fifo");

    // Ensure FIFOs exist (create if missing, using mkfifo command)
    for p in [&cmd_fifo, &resp_fifo] {
        if !p.exists() {
            let _ = fs::remove_file(p);
            let _ = std::process::Command::new("mkfifo")
                .arg(p.as_os_str())
                .output();
        }
    }

    // Spawn blocking task to listen on cmd_fifo and write responses to resp_fifo
    let cmd = cmd_fifo.clone();
    let resp = resp_fifo.clone();
    task::spawn_blocking(move || {
        loop {
            // open for read (blocking until writer appears)
            let f = match fs::OpenOptions::new().read(true).open(&cmd) {
                Ok(f) => f,
                Err(e) => { eprintln!("IPC reader open error: {}", e); std::thread::sleep(std::time::Duration::from_secs(1)); continue; }
            };
            let reader = BufReader::new(f);
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        // parse JSON command
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&l) {
                            if let Some(cmd_type) = v.get("cmd").and_then(|c| c.as_str()) {
                                match cmd_type {
                                    "mike_status" => {
                                        // Query system memory directly via sysinfo
                                        let mut sys = sysinfo::System::new();
                                        sys.refresh_memory();
                                        let resp_obj = json!({
                                            "status": "ok",
                                            "total_ram": sys.total_memory(),
                                            "used_ram": sys.used_memory(),
                                            "free_ram": sys.free_memory(),
                                        });
                                        if let Ok(mut of) = fs::OpenOptions::new().write(true).open(&resp) {
                                            let _ = writeln!(of, "{}", resp_obj.to_string());
                                        }
                                    }
                                    _ => {
                                        if let Ok(mut of) = fs::OpenOptions::new().write(true).open(&resp) {
                                            let _ = writeln!(of, "{}", json!({"status":"error","message":"unknown command"}));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => { eprintln!("IPC read error: {}", e); break; }
                }
            }
            // small delay before reopening
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });

    let mut orchestrator = Orchestrator::new(config).await?;

    // If CLI requested an IPC action, handle it and exit early
    if let Some(cmd) = &cli.command {
        use crate::saul::Commands;
        match cmd {
            Commands::Ipc { action } => {
                match action {
                    crate::saul::IpcAction::Send { fifo, payload } => {
                        let fifo = fifo.as_ref().map(PathBuf::from).unwrap_or(cmd_fifo);
                        // open write-only and send payload followed by newline
                        let mut of = fs::OpenOptions::new().write(true).open(&fifo)?;
                        writeln!(of, "{}", payload)?;
                        // read response
                        let mut rf = fs::OpenOptions::new().read(true).open(&resp_fifo)?;
                        let mut buf = String::new();
                        use std::io::Read;
                        rf.read_to_string(&mut buf)?;
                        println!("Response: {}", buf);
                        return Ok(());
                    }
                    crate::saul::IpcAction::Listen { .. } => {
                        println!("IPC listener started; use FIFO {} to send JSON commands.", cmd_fifo.display());
                        // Block the main thread while background listener runs
                        loop { std::thread::park(); }
                    }
                }
            }
            _ => {}
        }
    }

    orchestrator.run(cli).await?;

    Ok(())
}
