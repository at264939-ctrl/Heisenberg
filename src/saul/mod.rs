// saul/mod.rs — Config & Policy
// "Saul handles the fine print so the operation can run clean."

pub mod config;
pub mod policy;

pub use config::{Config, InferenceConfig};

use clap::{Parser, Subcommand};

/// Heisenberg — local autonomous AI agent
#[derive(Debug, Parser)]
#[command(
    name = "heisenberg",
    version = env!("CARGO_PKG_VERSION"),
    about = "Say my name.",
    long_about = "Heisenberg — a fully local AI agent running on GGUF models.\nNo cloud. No leaks. Just pure chemistry."
)]
pub struct Cli {
    /// Path to config file (defaults to ~/.config/heisenberg/config.toml)
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<String>,

    /// Enable verbose/debug output
    #[arg(short, long, default_value_t = false)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Start interactive REPL session
    Chat {
        /// Initial prompt to begin with
        #[arg(short, long)]
        prompt: Option<String>,
    },
    /// Run a one-shot task and exit
    Run {
        /// The task to execute
        task: String,
        /// Dry-run (plan only, no execution)
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Show memory and system status
    Status,
    /// Manage stored memories
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
    /// Execute a Bash script via Jesse
    Exec {
        /// Script path or inline command
        script: String,
        /// Run as inline shell command
        #[arg(long, default_value_t = false)]
        inline: bool,
    },
    /// Self-improvement operations
    Improve {
        /// Target module to analyze
        module: Option<String>,
    },
    /// JSON IPC helper for Bash <-> Rust communication
    Ipc {
        #[command(subcommand)]
        action: IpcAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum MemoryAction {
    /// Show recent interactions
    List {
        #[arg(short, long, default_value_t = 20)]
        limit: u32,
    },
    /// Compact and summarize old memories
    Compact,
    /// Clear all memories (destructive)
    Clear,
}

#[derive(Debug, Subcommand)]
pub enum IpcAction {
    /// Send a JSON message to the agent via FIFO and print a response
    Send {
        /// Path to FIFO (defaults to /tmp/heisenberg_cmd.fifo)
        #[arg(short, long)]
        fifo: Option<String>,
        /// JSON payload to send
        payload: String,
    },
    /// Start a listener (blocking) that listens for JSON commands and writes responses
    Listen {
        /// Command FIFO path (defaults to /tmp/heisenberg_cmd.fifo)
        #[arg(short, long)]
        cmd_fifo: Option<String>,
        /// Response FIFO path (defaults to /tmp/heisenberg_resp.fifo)
        #[arg(short, long)]
        resp_fifo: Option<String>,
    },
}
