#![allow(dead_code)]
// blue_sky/patcher.rs -- Applies patches after policy & test validation

use crate::jesse::JesseRunner;
use crate::saul::policy::PolicyEngine;
use anyhow::Result;
use tracing::{info, warn};

pub struct SelfPatcher {
    policy: PolicyEngine,
    runner: JesseRunner,
}

impl SelfPatcher {
    pub fn new(policy: PolicyEngine, runner: JesseRunner) -> Self {
        Self { policy, runner }
    }

    pub async fn apply_diff(&self, patch_content: &str) -> Result<()> {
        self.policy.check_self_modification()?;
        warn!("Blue Sky protocol initiated: Self-modification attempt.");

        let (dir, patch_path) = crate::jesse::sandbox::write_temp_script(patch_content)?;
        std::fs::rename(&patch_path, dir.path().join("update.patch"))?;
        let patch_file = dir.path().join("update.patch");

        // Validate patch syntax
        let val_script = format!("patch --dry-run -p1 < {}", patch_file.display());
        let res = self.runner.run_inline(&val_script).await?;
        if !res.success() {
            anyhow::bail!("Patch validation failed: {}", res.stderr);
        }

        // Apply
        let app_script = format!("patch -p1 < {}", patch_file.display());
        let res = self.runner.run_inline(&app_script).await?;
        if !res.success() {
            anyhow::bail!("Patch application failed: {}", res.stderr);
        }

        // Run tests
        info!("Running validation tests after patch...");
        let test_script = "cargo test";
        let res = self.runner.run_inline(test_script).await?;
        if !res.success() {
            warn!("Tests failed after patch! REVERTING.");
            let rev_script = format!("patch -R -p1 < {}", patch_file.display());
            let _ = self.runner.run_inline(&rev_script).await;
            anyhow::bail!("Self-modification reverted due to test failure.");
        }

        info!("Self-modification successful and verified.");
        Ok(())
    }
}
