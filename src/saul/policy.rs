#![allow(dead_code)]
// saul/policy.rs — Runtime policy enforcement
// "I know enough to know nothing — unless allowed."

use crate::saul::Config;
use anyhow::Result;
use tracing::warn;

/// Governs what the agent is permitted to do at runtime.
#[derive(Debug, Clone)]
pub struct PolicyEngine {
    /// Whether filesystem writes outside work_dir are allowed
    pub allow_external_writes: bool,
    /// Whether browser automation is permitted
    pub allow_browser: bool,
    /// Whether self-modification patches may be applied
    pub allow_self_modification: bool,
    /// Whether network access is permitted (always false for truly local mode)
    pub allow_network: bool,
    /// Max script execution time enforced by policy
    pub max_exec_secs: u64,
}

impl PolicyEngine {
    pub fn from_config(cfg: &Config) -> Self {
        Self {
            allow_external_writes: false,
            allow_browser: cfg.execution.browser_enabled,
            allow_self_modification: cfg.agent.self_improvement,
            allow_network: false,
            max_exec_secs: cfg.execution.script_timeout_secs,
        }
    }

    /// Check if a filesystem path is within the allowed work directory.
    pub fn check_path(&self, path: &std::path::Path, work_dir: &std::path::Path) -> Result<()> {
        if !self.allow_external_writes {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            let work = work_dir
                .canonicalize()
                .unwrap_or_else(|_| work_dir.to_path_buf());
            if !canonical.starts_with(&work) {
                anyhow::bail!(
                    "Policy violation: path '{}' is outside work dir '{}'",
                    path.display(),
                    work_dir.display()
                );
            }
        }
        Ok(())
    }

    /// Enforce browser policy.
    pub fn check_browser(&self) -> Result<()> {
        if !self.allow_browser {
            anyhow::bail!("Policy: browser automation is disabled. Enable via config.");
        }
        Ok(())
    }

    /// Enforce self-modification policy.
    pub fn check_self_modification(&self) -> Result<()> {
        if !self.allow_self_modification {
            anyhow::bail!(
                "Policy: self-modification is disabled. Set agent.self_improvement = true in config."
            );
        }
        warn!("⚠  Self-modification approved by policy. All changes will be recorded.");
        Ok(())
    }
}
