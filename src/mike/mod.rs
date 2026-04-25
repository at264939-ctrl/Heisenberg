#![allow(dead_code)]
// mike/mod.rs — Memory Manager
// "Mike doesn't forget. Mike never forgets."

pub mod db;
pub mod lru;
pub mod monitor;
pub mod zones;

pub use db::DbLedger;
pub use monitor::MemoryMonitor;
pub use zones::ZoneTracker;

use crate::saul::Config;
use anyhow::Result;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Global memory pressure level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MemoryPressure {
    /// < 60% of cap
    Normal,
    /// 60-80% of cap
    Elevated,
    /// 80-95% of cap — evict caches, reduce context
    High,
    /// > 95% of cap — emergency: drop non-essentials
    Critical,
}

impl MemoryPressure {
    pub fn from_usage_ratio(ratio: f64) -> Self {
        if ratio < 0.60 {
            Self::Normal
        } else if ratio < 0.80 {
            Self::Elevated
        } else if ratio < 0.95 {
            Self::High
        } else {
            Self::Critical
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            Self::Normal => "🟢",
            Self::Elevated => "🟡",
            Self::High => "🟠",
            Self::Critical => "🔴",
        }
    }
}

/// Mike — the memory manager.
pub struct Mike {
    pub max_bytes: u64,
    pub monitor: MemoryMonitor,
    pub zones: ZoneTracker,
    eviction_count: Arc<AtomicU64>,
    /// Handle for the background monitor task
    _bg_handle: Option<tokio::task::JoinHandle<()>>,
}

impl Mike {
    pub fn new(cfg: &Config) -> Self {
        let monitor = MemoryMonitor::new(cfg.agent.memory_poll_interval_ms);

        Self {
            max_bytes: cfg.agent.max_ram_bytes,
            monitor,
            zones: ZoneTracker::new(),
            eviction_count: Arc::new(AtomicU64::new(0)),
            _bg_handle: None,
        }
    }

    /// Start background memory monitoring loop.
    /// Call this once during initialization.
    pub fn start_background_monitor(mike: &Arc<Mike>) {
        let monitor = mike.monitor.clone_inner();
        let interval_ms = mike.monitor.poll_interval_ms();
        let max_bytes = mike.max_bytes;

        let handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(interval_ms)).await;
                monitor.refresh();
                let rss = monitor.current_rss_bytes();
                let pct = (rss as f64 / max_bytes as f64) * 100.0;

                if pct > 95.0 {
                    error!(
                        "CRITICAL: RSS={:.0}MB ({:.1}% of {:.0}MB cap)",
                        rss as f64 / 1e6,
                        pct,
                        max_bytes as f64 / 1e6
                    );
                } else if pct > 80.0 {
                    warn!("HIGH pressure: RSS={:.0}MB ({:.1}%)", rss as f64 / 1e6, pct);
                } else {
                    debug!("Memory: {:.0}MB ({:.1}%)", rss as f64 / 1e6, pct);
                }
            }
        });

        // We can't mutate through Arc, so just detach the task.
        // The task will be cancelled when the runtime shuts down.
        drop(handle);
    }

    /// Get current memory pressure level.
    pub fn pressure(&self) -> MemoryPressure {
        let rss = self.monitor.current_rss_bytes();
        let ratio = rss as f64 / self.max_bytes as f64;
        MemoryPressure::from_usage_ratio(ratio)
    }

    /// Return current RSS in bytes.
    pub fn rss_bytes(&self) -> u64 {
        self.monitor.current_rss_bytes()
    }

    /// Return current RSS as percentage of cap.
    pub fn usage_pct(&self) -> f64 {
        (self.rss_bytes() as f64 / self.max_bytes as f64) * 100.0
    }

    /// Assert we are below the hard cap. Returns Err if over.
    pub fn enforce_cap(&self) -> Result<()> {
        // Refresh before checking
        self.monitor.refresh();
        let rss = self.rss_bytes();
        if rss > self.max_bytes {
            error!(
                "MEMORY CAP BREACHED: {} MB used / {} MB allowed",
                rss / 1_048_576,
                self.max_bytes / 1_048_576
            );
            anyhow::bail!(
                "Memory cap exceeded: {} bytes > {} cap",
                rss,
                self.max_bytes
            );
        }
        Ok(())
    }

    /// Calculate available memory for inference (total budget minus current overhead)
    pub fn available_for_inference(&self) -> u64 {
        let rss = self.rss_bytes();
        let overhead = self
            .zones
            .total()
            .saturating_sub(self.zones.get(zones::MemoryZone::Inference));
        self.max_bytes.saturating_sub(overhead.max(rss))
    }

    /// Log a status line about current memory state.
    pub fn log_status(&self) {
        let rss = self.rss_bytes();
        let pct = self.usage_pct();
        let pressure = self.pressure();
        match pressure {
            MemoryPressure::Normal => {
                info!(
                    "Memory: {:.1} MB / {:.0} MB ({:.1}%) — Normal",
                    rss as f64 / 1e6,
                    self.max_bytes as f64 / 1e6,
                    pct
                );
            }
            MemoryPressure::Elevated => {
                info!(
                    "Memory: {:.1} MB / {:.0} MB ({:.1}%) — Elevated",
                    rss as f64 / 1e6,
                    self.max_bytes as f64 / 1e6,
                    pct
                );
            }
            MemoryPressure::High => {
                warn!(
                    "Memory: {:.1} MB / {:.0} MB ({:.1}%) — HIGH",
                    rss as f64 / 1e6,
                    self.max_bytes as f64 / 1e6,
                    pct
                );
            }
            MemoryPressure::Critical => {
                error!(
                    "Memory: {:.1} MB / {:.0} MB ({:.1}%) — CRITICAL",
                    rss as f64 / 1e6,
                    self.max_bytes as f64 / 1e6,
                    pct
                );
            }
        }
    }

    pub fn eviction_count(&self) -> u64 {
        self.eviction_count.load(Ordering::Relaxed)
    }

    pub fn record_eviction(&self) {
        self.eviction_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Returns recommended context size based on current pressure.
    pub fn recommended_context_size(&self, default: u32) -> u32 {
        // Refresh to get latest reading
        self.monitor.refresh();
        match self.pressure() {
            MemoryPressure::Normal => default,
            MemoryPressure::Elevated => (default as f64 * 0.75) as u32,
            MemoryPressure::High => (default as f64 * 0.5) as u32,
            MemoryPressure::Critical => (default as f64 * 0.25) as u32,
        }
    }

    /// Get a formatted status string for the UI
    pub fn status_line(&self) -> String {
        let rss_mb = self.rss_bytes() as f64 / 1_048_576.0;
        let max_mb = self.max_bytes as f64 / 1_048_576.0;
        let pct = self.usage_pct();
        let pressure = self.pressure();
        format!(
            "{} {:.0}/{:.0} MB ({:.1}%) {:?}",
            pressure.emoji(),
            rss_mb,
            max_mb,
            pct,
            pressure
        )
    }
}
