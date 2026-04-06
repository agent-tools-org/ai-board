// Core domain types: WorkItem, Status, Priority, Event, Board
// All shared types used across store, API, and CLI layers

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// Short hex ID for work items (e.g., "wi-a3f8")
pub type WorkItemId = String;

/// Short hex ID for boards (e.g., "bd-01")
pub type BoardId = String;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Backlog,
    Ready,
    Active,
    Review,
    Done,
    Blocked,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    pub id: WorkItemId,
    pub project: String,
    pub repo_path: String,
    pub title: String,
    pub description: String,
    pub status: Status,
    pub priority: Priority,
    pub position: f64,
    pub labels: Vec<String>,

    // Hierarchy
    pub parent_id: Option<WorkItemId>,
    pub depends_on: Vec<WorkItemId>,

    // Execution link
    pub aid_task_ids: Vec<String>,
    pub aid_agent: Option<String>,
    pub aid_verify: Option<String>,

    // Effort
    pub estimate: Option<String>,

    // Ownership
    pub assignee: Option<String>,
    pub created_by: String,

    // Gates
    pub requires_approval: bool,
    pub auto_dispatch: bool,

    // Timestamps
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
    pub started_at: Option<DateTime<Local>>,
    pub completed_at: Option<DateTime<Local>>,
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: i64,
    pub item_id: WorkItemId,
    pub actor: String,
    pub action: String,
    pub detail: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub id: BoardId,
    pub name: String,
    pub filter: Option<serde_json::Value>,
    pub sort_by: String,
    pub columns: Option<Vec<String>>,
    pub created_at: DateTime<Local>,
    pub updated_at: DateTime<Local>,
}
