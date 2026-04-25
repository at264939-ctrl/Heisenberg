// jesse/runner.rs -- Bash script execution engine

use super::output::ExecutionResult;
use crate::saul::policy::PolicyEngine;
use crate::saul::Config;
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, info, warn};

pub struct JesseRunner {
    work_dir: std::path::PathBuf,
    timeout_secs: u64,
    max_output_bytes: usize,
    policy: PolicyEngine,
}

impl JesseRunner {
    pub fn new(cfg: &Config) -> Self {
        Self {
            work_dir: cfg.execution.work_dir.clone(),
            timeout_secs: cfg.execution.script_timeout_secs,
            max_output_bytes: cfg.execution.max_output_bytes,
            policy: PolicyEngine::from_config(cfg),
        }
    }

    /// Execute an inline shell command.
    pub async fn run_inline(&self, cmd: &str) -> Result<ExecutionResult> {
        super::sandbox::validate_script(cmd)?;
        info!("Jesse executing inline: {}", &cmd[..cmd.len().min(80)]);
        let mut command = Command::new("bash");
        command
            .args(["-c", cmd])
            .current_dir(&self.work_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.run_command(command).await
    }

    /// Execute a Bash script file.
    pub async fn run_script(&self, script_path: &Path) -> Result<ExecutionResult> {
        self.policy.check_path(script_path, &self.work_dir)?;
        info!("Jesse executing script: {}", script_path.display());
        let mut command = Command::new("bash");
        command
            .arg(script_path)
            .current_dir(&self.work_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.run_command(command).await
    }

    async fn run_command(&self, mut cmd: Command) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();
        let dur = Duration::from_secs(self.timeout_secs);

        let output = timeout(dur, cmd.output())
            .await
            .context("Script execution timed out")?
            .context("Failed to run script")?;

        let elapsed = start.elapsed();
        let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if stdout.len() > self.max_output_bytes {
            warn!("stdout truncated ({} bytes)", stdout.len());
            stdout.truncate(self.max_output_bytes);
            stdout.push_str("\n... [truncated]");
        }
        if stderr.len() > self.max_output_bytes {
            stderr.truncate(self.max_output_bytes);
            stderr.push_str("\n... [truncated]");
        }

        let result = ExecutionResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout,
            stderr,
            elapsed_ms: elapsed.as_millis() as u64,
        };

        debug!(
            "Jesse result: exit={} elapsed={}ms",
            result.exit_code, result.elapsed_ms
        );
        Ok(result)
    }
}
