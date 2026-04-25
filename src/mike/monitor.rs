#![allow(dead_code)]
// mike/monitor.rs — RSS and system memory polling

use std::sync::{Arc, RwLock};
use sysinfo::System;
use tracing::debug;

/// Polls system RSS for this process and caches results.
pub struct MemoryMonitor {
    poll_interval_ms: u64,
    inner: Arc<MonitorInner>,
}

struct MonitorInner {
    cached_rss: RwLock<u64>,
}

impl MemoryMonitor {
    pub fn new(poll_interval_ms: u64) -> Self {
        let inner = Arc::new(MonitorInner {
            cached_rss: RwLock::new(Self::sample_rss()),
        });
        Self {
            poll_interval_ms,
            inner,
        }
    }

    /// Sample current process RSS directly (blocking).
    fn sample_rss() -> u64 {
        let pid = sysinfo::Pid::from_u32(std::process::id());
        let mut sys = System::new();
        sys.refresh_all();
        sys.process(pid).map(|p| p.memory()).unwrap_or(0)
    }

    /// Return cached RSS (refreshed by background task or on-demand).
    pub fn current_rss_bytes(&self) -> u64 {
        *self
            .inner
            .cached_rss
            .read()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// Refresh the cache now.
    pub fn refresh(&self) {
        let rss = Self::sample_rss();
        debug!("RSS refresh: {} bytes ({:.1} MB)", rss, rss as f64 / 1e6);
        let mut guard = self
            .inner
            .cached_rss
            .write()
            .unwrap_or_else(|e| e.into_inner());
        *guard = rss;
    }

    pub fn poll_interval_ms(&self) -> u64 {
        self.poll_interval_ms
    }

    /// Clone the inner Arc for use in background tasks.
    pub fn clone_inner(&self) -> MemoryMonitorHandle {
        MemoryMonitorHandle {
            inner: self.inner.clone(),
        }
    }
}

/// A lightweight handle for background monitoring tasks
pub struct MemoryMonitorHandle {
    inner: Arc<MonitorInner>,
}

impl MemoryMonitorHandle {
    pub fn current_rss_bytes(&self) -> u64 {
        *self
            .inner
            .cached_rss
            .read()
            .unwrap_or_else(|e| e.into_inner())
    }

    pub fn refresh(&self) {
        let rss = MemoryMonitor::sample_rss();
        let mut guard = self
            .inner
            .cached_rss
            .write()
            .unwrap_or_else(|e| e.into_inner());
        *guard = rss;
    }
}
