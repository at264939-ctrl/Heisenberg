#![allow(dead_code)]
// mike/zones.rs — Memory zone tracking

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Named memory zones for budget tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryZone {
    Inference,
    Cache,
    State,
    ScreenBuffer,
    BrowserSession,
}

impl std::fmt::Display for MemoryZone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryZone::Inference => write!(f, "inference"),
            MemoryZone::Cache => write!(f, "cache"),
            MemoryZone::State => write!(f, "state"),
            MemoryZone::ScreenBuffer => write!(f, "screen_buffer"),
            MemoryZone::BrowserSession => write!(f, "browser_session"),
        }
    }
}

/// Tracks per-zone estimated memory usage.
pub struct ZoneTracker {
    inference: Arc<AtomicU64>,
    cache: Arc<AtomicU64>,
    state: Arc<AtomicU64>,
    screen: Arc<AtomicU64>,
    browser: Arc<AtomicU64>,
}

impl ZoneTracker {
    pub fn new() -> Self {
        Self {
            inference: Arc::new(AtomicU64::new(0)),
            cache: Arc::new(AtomicU64::new(0)),
            state: Arc::new(AtomicU64::new(0)),
            screen: Arc::new(AtomicU64::new(0)),
            browser: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn set(&self, zone: MemoryZone, bytes: u64) {
        self.counter(zone).store(bytes, Ordering::Relaxed);
    }

    pub fn get(&self, zone: MemoryZone) -> u64 {
        self.counter(zone).load(Ordering::Relaxed)
    }

    pub fn total(&self) -> u64 {
        [
            MemoryZone::Inference,
            MemoryZone::Cache,
            MemoryZone::State,
            MemoryZone::ScreenBuffer,
            MemoryZone::BrowserSession,
        ]
        .iter()
        .map(|z| self.get(*z))
        .sum()
    }

    pub fn clear(&self, zone: MemoryZone) {
        self.counter(zone).store(0, Ordering::Relaxed);
    }

    fn counter(&self, zone: MemoryZone) -> &AtomicU64 {
        match zone {
            MemoryZone::Inference => &self.inference,
            MemoryZone::Cache => &self.cache,
            MemoryZone::State => &self.state,
            MemoryZone::ScreenBuffer => &self.screen,
            MemoryZone::BrowserSession => &self.browser,
        }
    }

    pub fn report(&self) -> String {
        format!(
            "inference={:.1}MB cache={:.1}MB state={:.1}MB screen={:.1}MB browser={:.1}MB",
            self.get(MemoryZone::Inference) as f64 / 1e6,
            self.get(MemoryZone::Cache) as f64 / 1e6,
            self.get(MemoryZone::State) as f64 / 1e6,
            self.get(MemoryZone::ScreenBuffer) as f64 / 1e6,
            self.get(MemoryZone::BrowserSession) as f64 / 1e6,
        )
    }
}

impl Default for ZoneTracker {
    fn default() -> Self {
        Self::new()
    }
}
