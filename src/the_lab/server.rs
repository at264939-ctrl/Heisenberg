// the_lab/server.rs -- llama-cpp server process management with TurboQuant
// "This is not meth. This is art."

use crate::the_lab::gguf::TurboQuantPlan;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tracing::{info, warn};

pub struct LlamaServer {
    bin: PathBuf,
    model: PathBuf,
    port: u16,
    threads: u32,
    gpu_layers: u32,
    turbo_plan: TurboQuantPlan,
    child: Option<Child>,
}

impl LlamaServer {
    pub fn new(
        bin: PathBuf,
        model: PathBuf,
        port: u16,
        threads: u32,
        gpu_layers: u32,
        turbo_plan: TurboQuantPlan,
    ) -> Self {
        Self {
            bin,
            model,
            port,
            threads,
            gpu_layers,
            turbo_plan,
            child: None,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        if self.child.is_some() {
            return Ok(());
        }

        let plan = &self.turbo_plan;

        info!("Starting llama-server with TurboQuant: {}", plan);

        let mut cmd = Command::new(&self.bin);
        cmd.args([
            "--model",
            self.model.to_str().unwrap_or("model.gguf"),
            "--ctx-size",
            &plan.context_length.to_string(),
            "--threads",
            &self.threads.to_string(),
            "--batch-size",
            &plan.batch_size.to_string(),
            "--n-gpu-layers",
            &self.gpu_layers.to_string(),
            "--port",
            &self.port.to_string(),
            "--host",
            "127.0.0.1",
            // ── TurboQuant KV Cache Quantization ──
            "--cache-type-k",
            &plan.kv_cache_k_type,
            "--cache-type-v",
            &plan.kv_cache_v_type,
            "--log-disable",
            "--no-warmup",
        ]);

        // Flash attention for memory efficiency (TurboQuant requirement)
        if plan.flash_attn {
            cmd.args(["--flash-attn", "on"]);
        }

        // Memory mapping control
        if !plan.mmap {
            cmd.arg("--no-mmap");
        }
        if plan.mlock {
            cmd.arg("--mlock");
        }

        cmd.stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);

        let child = cmd
            .spawn()
            .context("Failed to spawn llama-cpp server process")?;
        info!("llama-server spawned (PID: {:?}) | ctx={} K={} V={} batch={} flash={} mmap={} mlock={}",
            child.id(), plan.context_length,
            plan.kv_cache_k_type, plan.kv_cache_v_type,
            plan.batch_size, plan.flash_attn, plan.mmap, plan.mlock);
        self.child = Some(child);
        Ok(())
    }

    pub async fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            if let Err(e) = child.kill().await {
                warn!("Failed to kill llama-server: {}", e);
            } else {
                info!("llama-server stopped.");
            }
        }
    }

    /// Returns the TurboQuant plan being used
    pub fn turbo_plan(&self) -> &TurboQuantPlan {
        &self.turbo_plan
    }
}

impl Drop for LlamaServer {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
        }
    }
}
