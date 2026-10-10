//! Background Tasks
//!
//! GCP API calls run in spawned tokio tasks so the UI keeps rendering and
//! reading keys while they are in flight. Each task reports back to the main
//! loop with a [`TaskResult`] over an unbounded channel; the loop applies it
//! to the [`App`](crate::app::App) state.

use crate::gcp::client::OperationStatus;
use crate::resource::PaginatedResult;
use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

/// Maximum number of concurrent requests for bulk actions
pub const BULK_ACTION_CONCURRENCY: usize = 8;

pub type TaskSender = UnboundedSender<TaskResult>;
pub type TaskReceiver = UnboundedReceiver<TaskResult>;

/// Create the channel used by background tasks to report results
pub fn channel() -> (TaskSender, TaskReceiver) {
    tokio::sync::mpsc::unbounded_channel()
}

/// Result of a background task, applied to the app state by the main loop
pub enum TaskResult {
    /// A page of resources was fetched
    Page {
        /// Fetch generation; results from superseded fetches are dropped
        generation: u64,
        result: anyhow::Result<PaginatedResult>,
    },
    /// Monitoring metrics for the VM instances of a fetched page
    Metrics {
        generation: u64,
        result: anyhow::Result<serde_json::Map<String, Value>>,
    },
    /// One or more resource actions completed (one entry per resource)
    Actions {
        results: Vec<(Uuid, anyhow::Result<Value>)>,
        /// Whether the actions were started as a bulk operation
        bulk: bool,
    },
    /// A long-running operation was polled
    OperationPolled {
        notification_id: Uuid,
        result: anyhow::Result<OperationStatus>,
    },
}

/// Send a task result, ignoring the error if the main loop has exited
pub fn send(tx: &TaskSender, result: TaskResult) {
    let _ = tx.send(result);
}
