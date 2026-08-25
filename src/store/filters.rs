// Query and update shapes shared by the store's item helpers.
// Kept apart from items.rs so the query code has room to stay readable.

use crate::types::{Priority, Status};

#[derive(Default)]
pub struct ItemFilter {
    pub status: Option<Status>,
    pub priority: Option<Priority>,
    pub label: Option<String>,
    pub assignee: Option<String>,
    pub parent_id: Option<String>,
    pub project: Option<String>,
    pub repo_path: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Default)]
pub struct ItemUpdate {
    pub project: Option<String>,
    pub repo_path: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<Priority>,
    pub position: Option<f64>,
    pub labels: Option<Vec<String>>,
    pub parent_id: Option<Option<String>>,
    pub depends_on: Option<Vec<String>>,
    pub aid_task_ids: Option<Vec<String>>,
    pub aid_agent: Option<Option<String>>,
    pub aid_verify: Option<Option<String>>,
    pub estimate: Option<Option<String>>,
    pub assignee: Option<Option<String>>,
    pub requires_approval: Option<bool>,
    pub auto_dispatch: Option<bool>,
    pub due_date: Option<Option<String>>,
}
