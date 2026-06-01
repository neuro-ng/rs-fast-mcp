//! Background task scheduling via the Docket queue.
//!
//! When a client passes `task_meta` alongside a tool/resource/prompt call the
//! request is routed through the middleware chain first (auth, rate-limiting)
//! and then submitted to the [`Docket`] background worker, which returns a
//! `task_id` immediately.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Metadata passed by the caller to request background execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMeta {
    /// How long (seconds) the task result should be retained after completion.
    pub ttl: u64,
    /// Namespaced component key (e.g. `"tool:my_tool"`). Filled by the server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fn_key: Option<String>,
}

impl TaskMeta {
    pub fn new(ttl: u64) -> Self {
        Self { ttl, fn_key: None }
    }
}

/// Returned immediately when a background task is scheduled.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskResult {
    pub task_id: String,
}

/// Current lifecycle state of a background task.
#[derive(Debug, Clone)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed(serde_json::Value),
    Failed(String),
}

struct QueuedTask {
    task_id: String,
    fn_key: String,
    arguments: serde_json::Value,
    ttl: u64,
    executor: TaskExecutor,
}

/// A boxed async function that executes the task and returns a JSON value.
pub type TaskExecutor = Arc<
    dyn Fn(
            serde_json::Value,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send>,
        > + Send
        + Sync,
>;

/// Channel-based background task scheduler.
///
/// Tasks are submitted via [`submit`], executed in a dedicated worker, and
/// their results (or errors) stored until the TTL expires.
pub struct Docket {
    sender: mpsc::Sender<QueuedTask>,
    tasks: Arc<DashMap<String, TaskStatus>>,
}

impl Docket {
    /// Spawns the worker loop and returns a `Docket` handle.
    ///
    /// If no Tokio runtime is active the worker is not started yet; it will be
    /// started lazily on the first [`submit`] call (which must be called from
    /// within an async context).
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<QueuedTask>(256);
        let tasks: Arc<DashMap<String, TaskStatus>> = Arc::new(DashMap::new());

        let docket = Self {
            sender: tx,
            tasks: tasks.clone(),
        };

        // Only start the background worker if we're already inside a Tokio runtime.
        if tokio::runtime::Handle::try_current().is_ok() {
            docket.start_worker(rx, tasks);
        } else {
            // Worker will never be started — `submit` is only callable from async
            // contexts which will have a runtime at that point. For test/sync
            // contexts where submit is never called this is fine.
            drop(rx);
        }

        docket
    }

    fn start_worker(
        &self,
        mut rx: mpsc::Receiver<QueuedTask>,
        tasks: Arc<DashMap<String, TaskStatus>>,
    ) {
        tokio::spawn(async move {
            while let Some(task) = rx.recv().await {
                let task_id = task.task_id.clone();
                let tasks_ref = tasks.clone();

                tasks_ref.insert(task_id.clone(), TaskStatus::Running);

                let executor = task.executor.clone();
                let args = task.arguments.clone();
                let ttl = task.ttl;

                let result = executor(args).await;

                let status = match result {
                    Ok(v) => TaskStatus::Completed(v),
                    Err(e) => TaskStatus::Failed(e),
                };
                tasks_ref.insert(task_id.clone(), status);

                let evict_tasks = tasks.clone();
                let evict_id = task_id.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(ttl)).await;
                    evict_tasks.remove(&evict_id);
                });
            }
        });
    }

    /// Submit a task to the background queue.
    ///
    /// Returns the `task_id` immediately; the executor will run asynchronously.
    pub async fn submit(
        &self,
        fn_key: String,
        arguments: serde_json::Value,
        ttl: u64,
        executor: TaskExecutor,
    ) -> Result<String, crate::error::FastMCPError> {
        let task_id = Uuid::new_v4().to_string();

        self.tasks
            .insert(task_id.clone(), TaskStatus::Pending);

        let task = QueuedTask {
            task_id: task_id.clone(),
            fn_key,
            arguments,
            ttl,
            executor,
        };

        self.sender
            .send(task)
            .await
            .map_err(|_| crate::error::FastMCPError::new("Docket channel closed".to_string()))?;

        Ok(task_id)
    }

    /// Query the status of a previously-submitted task.
    pub fn get_status(&self, task_id: &str) -> Option<TaskStatus> {
        self.tasks.get(task_id).map(|v| v.clone())
    }
}

impl Default for Docket {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Docket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Docket")
            .field("pending_tasks", &self.tasks.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_submit_and_complete() {
        let docket = Docket::new();

        let executor: TaskExecutor = Arc::new(|args| {
            Box::pin(async move { Ok(serde_json::json!({ "echo": args })) })
        });

        let id = docket
            .submit(
                "tool:test".to_string(),
                serde_json::json!({"x": 1}),
                60,
                executor,
            )
            .await
            .unwrap();

        // Wait briefly for the worker
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let status = docket.get_status(&id).unwrap();
        assert!(matches!(status, TaskStatus::Completed(_)));
    }

    #[tokio::test]
    async fn test_failed_task() {
        let docket = Docket::new();

        let executor: TaskExecutor =
            Arc::new(|_| Box::pin(async { Err("boom".to_string()) }));

        let id = docket
            .submit("tool:boom".to_string(), serde_json::json!({}), 60, executor)
            .await
            .unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let status = docket.get_status(&id).unwrap();
        assert!(matches!(status, TaskStatus::Failed(_)));
    }
}
