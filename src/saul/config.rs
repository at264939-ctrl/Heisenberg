#![allow(dead_code)]
// saul/config.rs — System configuration loader

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::{Path, PathBuf};

/// Top-level configuration for the Heisenberg system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub agent: AgentConfig,
    pub memory: MemoryConfig,
    pub inference: InferenceConfig,
    pub execution: ExecutionConfig,
    pub database: DatabaseConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Agent name (thematic, can be changed)
    pub name: String,
    /// Max RAM in bytes (hard limit: 3 GB = 3_221_225_472)
    pub max_ram_bytes: u64,
    /// How often (ms) to poll memory usage
    pub memory_poll_interval_ms: u64,
    /// Enable self-improvement (blue_sky module)
    pub self_improvement: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Max entries in hot LRU cache
    pub lru_capacity: usize,
    /// Compact memories older than this many days
    pub compact_after_days: u32,
    /// Max summary length in characters
    pub max_summary_chars: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    /// Path to GGUF model file OR directory containing .gguf files.
    /// If a directory, the engine will auto-scan and select the best model.
    pub model_path: PathBuf,
    /// llama-cpp server binary path
    pub llama_server_bin: PathBuf,
    /// llama-cpp server port
    pub server_port: u16,
    /// Max context length (tokens) — used as upper bound; TurboQuant may reduce
    pub context_length: u32,
    /// Number of CPU threads for inference
    pub threads: u32,
    /// Batch size for prompt processing — may be overridden by TurboQuant
    pub batch_size: u32,
    /// GPU layers to offload (0 = CPU only)
    pub gpu_layers: u32,
    /// Max tokens to generate per request
    pub max_tokens: u32,
    /// Temperature for sampling
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    /// Working directory for script execution
    pub work_dir: PathBuf,
    /// Timeout for Bash script execution (seconds)
    pub script_timeout_secs: u64,
    /// Max script output size (bytes)
    pub max_output_bytes: usize,
    /// Enable browser automation
    pub browser_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub name: String,
    pub user: String,
    pub password: String,
    pub pool_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        // Auto-detect model path: prefer models/ directory for auto-scanning
        let models_dir = cwd.join("models");
        let model_path = if models_dir.exists() {
            models_dir
        } else {
            // Fallback to a specific file
            cwd.join("models/model.gguf")
        };

        // Auto-detect llama-server binary
        let llama_bin = Self::find_llama_server(&cwd);

        // Auto-detect optimal thread count (keep it low to prevent page thrashing under memory constraints)
        let threads = std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(2)
            .min(2); // cap at 2 to prevent extreme mmap disk thrashing

        Self {
            agent: AgentConfig {
                name: "Heisenberg".into(),
                max_ram_bytes: 3_221_225_472, // 3 GB
                memory_poll_interval_ms: 2_000,
                self_improvement: false,
            },
            memory: MemoryConfig {
                lru_capacity: 512,
                compact_after_days: 30,
                max_summary_chars: 2048,
            },
            inference: InferenceConfig {
                model_path,
                llama_server_bin: llama_bin,
                server_port: 34884,
                context_length: 4_096,
                threads,
                batch_size: 512,
                gpu_layers: 0,
                max_tokens: 1_024,
                temperature: 0.7,
            },
            execution: ExecutionConfig {
                work_dir: cwd,
                script_timeout_secs: 30,
                max_output_bytes: 65_536,
                browser_enabled: false,
            },
            database: DatabaseConfig {
                host: "localhost".into(),
                port: 5432,
                name: "heisenberg".into(),
                user: "heisenberg".into(),
                password: "blue_sky".into(),
                pool_size: 4,
            },
        }
    }
}

impl Config {
    /// Find llama-server binary in common locations
    fn find_llama_server(cwd: &Path) -> PathBuf {
        let candidates = [
            cwd.join("bin/llama-server"),
            PathBuf::from("/usr/local/bin/llama-server"),
            PathBuf::from("/usr/bin/llama-server"),
        ];

        // Also check PATH
        if let Ok(output) = std::process::Command::new("which")
            .arg("llama-server")
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return PathBuf::from(path);
                }
            }
        }

        for p in &candidates {
            if p.exists() {
                return p.clone();
            }
        }

        // Default fallback
        cwd.join("bin/llama-server")
    }

    /// Load config from file, falling back to defaults.
    pub fn load(path: Option<&str>) -> Result<Self> {
        let config_path = match path {
            Some(p) => PathBuf::from(p),
            None => {
                let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                home.join(".config").join("heisenberg").join("config.toml")
            }
        };

        if config_path.exists() {
            let contents = std::fs::read_to_string(&config_path)
                .with_context(|| format!("Failed to read config: {}", config_path.display()))?;
            let cfg: Config = toml::from_str(&contents)
                .with_context(|| format!("Failed to parse config: {}", config_path.display()))?;
            Ok(cfg)
        } else {
            tracing::warn!(
                "Config not found at {}. Using defaults.",
                config_path.display()
            );
            Ok(Config::default())
        }
    }

    /// Write current config to file.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }
}
