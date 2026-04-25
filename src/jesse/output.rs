#![allow(dead_code)]
// jesse/output.rs -- Structured execution result

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub elapsed_ms: u64,
}

impl ExecutionResult {
    pub fn success(&self) -> bool {
        self.exit_code == 0
    }

    pub fn combined_output(&self) -> String {
        if self.stderr.is_empty() {
            self.stdout.clone()
        } else {
            format!("{}\n[stderr]: {}", self.stdout, self.stderr)
        }
    }

    pub fn summary(&self) -> String {
        let status = if self.success() { "OK" } else { "FAIL" };
        format!(
            "[{}] exit={} elapsed={}ms stdout_len={}",
            status,
            self.exit_code,
            self.elapsed_ms,
            self.stdout.len(),
        )
    }
}
