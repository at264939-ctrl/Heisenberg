#![allow(dead_code)]
// gus/scheduler.rs -- Task queue and scheduler

use super::task::Task;
use anyhow::Result;
use flume::{Receiver, Sender};
use tracing::{info, warn};

pub struct GusScheduler {
    tx: Sender<Task>,
    rx: Receiver<Task>,
    max_queue: usize,
}

impl GusScheduler {
    pub fn new(max_queue: usize) -> Self {
        let (tx, rx) = flume::bounded(max_queue);
        Self { tx, rx, max_queue }
    }

    /// Enqueue a task. Returns Err if queue is full.
    pub fn enqueue(&self, task: Task) -> Result<()> {
        if self.tx.len() >= self.max_queue {
            warn!(
                "Gus: task queue full ({} items). Dropping task '{}'.",
                self.max_queue, task.description
            );
            anyhow::bail!("Task queue is full — cannot enqueue '{}'", task.description);
        }
        info!("Gus: queued task '{}' [{}]", task.description, task.id);
        self.tx.send(task)?;
        Ok(())
    }

    /// Receive the next task (async, blocking).
    pub async fn next(&self) -> Option<Task> {
        self.rx.recv_async().await.ok()
    }

    /// Try to receive the next task without blocking.
    pub fn try_next(&self) -> Option<Task> {
        self.rx.try_recv().ok()
    }

    pub fn queue_len(&self) -> usize {
        self.rx.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rx.is_empty()
    }
}
