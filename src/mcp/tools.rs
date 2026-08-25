// MCP tool handlers for ai-board work item operations.
// Exports: McpServer and board_* tools over rmcp stdio transport.
// Deps: crate::store CRUD helpers, crate::types, rmcp server macros.

use std::sync::Arc;

use anyhow::{Result, anyhow, bail};
use chrono::Local;
use rmcp::{
    ServerHandler, schemars, tool,
    handler::server::tool::Parameters,
    model::{Implementation, ServerInfo},
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    store::{
        ItemFilter, ItemUpdate, Store, gen_artifact_id, gen_id, get_item, insert_artifact,
        insert_event, insert_item, list_artifacts, list_events, list_items, next_item,
        update_item, update_item_status,
    },
    types::{Artifact, ArtifactType, Event, Priority, Status, WorkItem},
};

const MCP_ACTOR: &str = "agent:mcp";

#[derive(Clone)]
pub struct McpServer {
    pub store: Arc<Store>,
}

impl McpServer {
    pub(crate) fn new(store: Arc<Store>) -> Self {
        Self { store }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub(crate) struct BoardNextParams { pub project: Option<String>, pub label: Option<String> }
#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub(crate) struct BoardListParams { pub project: Option<String>, pub status: Option<String>, pub priority: Option<String>, pub label: Option<String>, pub limit: Option<usize> }
#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub(crate) struct BoardShowParams { pub id: String }
#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub(crate) struct BoardArtifactsParams { pub item_id: String }
#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub(crate) struct BoardCreateParams { pub project: Option<String>, pub title: String, pub description: Option<String>, pub priority: Option<String>, pub labels: Option<Vec<String>>, pub depends_on: Option<Vec<String>> }
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct BoardAttachParams { pub item_id: String, pub artifact_type: ArtifactType, pub title: String, pub path: Option<String>, pub content: Option<String>, pub status: Option<String> }
#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub(crate) struct BoardClaimParams { pub id: String, pub assignee: Option<String> }
#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub(crate) struct BoardCompleteParams { pub id: String, pub summary: String, pub aid_task_id: Option<String> }
#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub(crate) struct BoardBlockParams { pub id: String, pub reason: String }
#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub(crate) struct BoardSubmitParams { pub id: String, pub summary: Option<String> }
#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub(crate) struct BoardNoteParams { pub id: String, pub note: String }
#[derive(Debug, Deserialize, schemars::JsonSchema, Default)]
pub(crate) struct BoardUpdateParams { pub id: String, pub project: Option<String>, pub title: Option<String>, pub description: Option<String>, pub priority: Option<String>, pub labels: Option<Vec<String>> }

#[derive(Serialize)]
struct ItemWithEvents { item: WorkItem, events: Vec<Event> }

#[tool(tool_box)]
impl McpServer {
    #[tool(name = "board_next", description = "Get the highest-priority ready item for a project")]
    pub(crate) async fn board_next(&self, #[tool(aggr)] Parameters(params): Parameters<BoardNextParams>) -> Result<String, String> {
        let project = params.project.unwrap_or_else(detect_project);
        self.store.with_connection(|conn| match next_item(conn, &project, params.label.as_deref())? {
            Some(item) => pretty_json(&item),
            None => Ok(format!("No ready items in project '{project}'")),
        }).map_err(tool_error)
    }

    #[tool(name = "board_list", description = "List work items for a project with optional filters")]
    pub(crate) async fn board_list(&self, #[tool(aggr)] Parameters(params): Parameters<BoardListParams>) -> Result<String, String> {
        let filter = ItemFilter {
            project: Some(params.project.unwrap_or_else(detect_project)),
            status: parse_status(params.status).map_err(tool_error)?,
            priority: parse_priority(params.priority).map_err(tool_error)?,
            label: params.label,
            assignee: None,
            parent_id: None,
            repo_path: None,
            limit: params.limit,
        };
        self.store.with_connection(|conn| pretty_json(&list_items(conn, &filter)?)).map_err(tool_error)
    }

    #[tool(name = "board_show", description = "Show one work item with recent events")]
    pub(crate) async fn board_show(&self, #[tool(aggr)] Parameters(params): Parameters<BoardShowParams>) -> Result<String, String> {
        self.store.with_connection(|conn| {
            let item = fetch_item(conn, &params.id)?;
            let events = list_events(conn, &params.id, Some(20))?;
            pretty_json(&ItemWithEvents { item, events })
        }).map_err(tool_error)
    }

    #[tool(name = "board_artifacts", description = "List artifacts attached to a work item")]
    pub(crate) async fn board_artifacts(&self, #[tool(aggr)] Parameters(params): Parameters<BoardArtifactsParams>) -> Result<String, String> {
        self.store.with_connection(|conn| { fetch_item(conn, &params.item_id)?; pretty_json(&list_artifacts(conn, &params.item_id)?) }).map_err(tool_error)
    }

    #[tool(name = "board_create", description = "Create a new work item for a project")]
    pub(crate) async fn board_create(&self, #[tool(aggr)] Parameters(params): Parameters<BoardCreateParams>) -> Result<String, String> {
        if params.title.trim().is_empty() {
            return Err("title cannot be empty".to_owned());
        }
        let now = Local::now();
        let item = WorkItem {
            id: gen_id(),
            project: params.project.unwrap_or_else(detect_project),
            repo_path: repo_path().map_err(tool_error)?,
            title: params.title,
            description: params.description.unwrap_or_default(),
            status: Status::Backlog,
            priority: parse_priority(params.priority).map_err(tool_error)?.unwrap_or(Priority::Medium),
            position: now.timestamp_millis() as f64,
            labels: params.labels.unwrap_or_default(),
            parent_id: None,
            depends_on: params.depends_on.unwrap_or_default(),
            aid_task_ids: Vec::new(),
            aid_agent: None,
            aid_verify: None,
            estimate: None,
            assignee: None,
            created_by: MCP_ACTOR.to_owned(),
            requires_approval: false,
            auto_dispatch: false,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            due_date: None,
        };
        self.store.with_connection(|conn| {
            insert_item(conn, &item)?;
            insert_event(conn, &item.id, MCP_ACTOR, "created", Some(&item.title), None)?;
            pretty_json(&item)
        }).map_err(tool_error)
    }

    #[tool(name = "board_attach", description = "Attach an artifact to a work item")]
    pub(crate) async fn board_attach(&self, #[tool(aggr)] Parameters(params): Parameters<BoardAttachParams>) -> Result<String, String> {
        if params.title.trim().is_empty() {
            return Err("artifact title cannot be empty".to_owned());
        }
        let now = Local::now();
        let artifact = Artifact { id: gen_artifact_id(), item_id: params.item_id.clone(), artifact_type: params.artifact_type, title: params.title, path: params.path, content: params.content.unwrap_or_default(), status: params.status.unwrap_or_else(|| "final".to_owned()), created_by: MCP_ACTOR.to_owned(), created_at: now, updated_at: now };
        self.store.with_connection(|conn| { fetch_item(conn, &params.item_id)?; insert_artifact(conn, &artifact)?; pretty_json(&artifact) }).map_err(tool_error)
    }

    #[tool(name = "board_claim", description = "Claim a work item and mark it active")]
    pub(crate) async fn board_claim(&self, #[tool(aggr)] Parameters(params): Parameters<BoardClaimParams>) -> Result<String, String> {
        let assignee = normalize_assignee(params.assignee).map_err(tool_error)?;
        self.store.with_connection(|conn| {
            let item = fetch_item(conn, &params.id)?;
            if assignee.is_some() {
                update_item(conn, &params.id, &ItemUpdate { assignee: assignee.clone().map(Some), ..ItemUpdate::default() })?;
            }
            update_item_status(conn, &params.id, Status::Active, false)?;
            insert_event(conn, &params.id, assignee.as_deref().unwrap_or(&current_actor(&item)), "claimed", None, None)?;
            pretty_json(&fetch_item(conn, &params.id)?)
        }).map_err(tool_error)
    }

    #[tool(name = "board_complete", description = "Complete a work item with a summary")]
    pub(crate) async fn board_complete(&self, #[tool(aggr)] Parameters(params): Parameters<BoardCompleteParams>) -> Result<String, String> {
        if params.summary.trim().is_empty() {
            return Err("summary cannot be empty".to_owned());
        }
        self.store.with_connection(|conn| {
            let mut item = fetch_item(conn, &params.id)?;
            if let Some(task_id) = params.aid_task_id.as_deref() {
                push_aid_task(&mut item, task_id);
                update_item(conn, &params.id, &ItemUpdate { aid_task_ids: Some(item.aid_task_ids.clone()), ..ItemUpdate::default() })?;
            }
            update_item_status(conn, &params.id, if item.requires_approval { Status::Review } else { Status::Done }, false)?;
            let metadata = json!({ "summary": params.summary, "aid_task_id": params.aid_task_id });
            insert_event(conn, &params.id, &current_actor(&item), "completed", Some(&params.summary), Some(&metadata))?;
            pretty_json(&fetch_item(conn, &params.id)?)
        }).map_err(tool_error)
    }

    #[tool(name = "board_block", description = "Mark a work item blocked with a reason")]
    pub(crate) async fn board_block(&self, #[tool(aggr)] Parameters(params): Parameters<BoardBlockParams>) -> Result<String, String> {
        if params.reason.trim().is_empty() {
            return Err("reason cannot be empty".to_owned());
        }
        self.store.with_connection(|conn| {
            let item = fetch_item(conn, &params.id)?;
            update_item_status(conn, &params.id, Status::Blocked, false)?;
            insert_event(conn, &params.id, &current_actor(&item), "blocked", Some(&params.reason), None)?;
            ack(&params.id, "blocked")
        }).map_err(tool_error)
    }

    #[tool(name = "board_submit", description = "Submit a work item for human review")]
    pub(crate) async fn board_submit(&self, #[tool(aggr)] Parameters(params): Parameters<BoardSubmitParams>) -> Result<String, String> {
        self.store.with_connection(|conn| {
            let item = fetch_item(conn, &params.id)?;
            update_item_status(conn, &params.id, Status::Review, false)?;
            insert_event(conn, &params.id, &current_actor(&item), "submitted", params.summary.as_deref().filter(|s| !s.trim().is_empty()), None)?;
            ack(&params.id, "submitted")
        }).map_err(tool_error)
    }

    #[tool(name = "board_note", description = "Add a progress note to a work item")]
    pub(crate) async fn board_note(&self, #[tool(aggr)] Parameters(params): Parameters<BoardNoteParams>) -> Result<String, String> {
        if params.note.trim().is_empty() {
            return Err("note cannot be empty".to_owned());
        }
        self.store.with_connection(|conn| {
            fetch_item(conn, &params.id)?;
            insert_event(conn, &params.id, MCP_ACTOR, "note", Some(&params.note), None)?;
            ack(&params.id, "noted")
        }).map_err(tool_error)
    }

    #[tool(name = "board_update", description = "Update editable work item fields")]
    pub(crate) async fn board_update(&self, #[tool(aggr)] Parameters(params): Parameters<BoardUpdateParams>) -> Result<String, String> {
        let priority = parse_priority(params.priority).map_err(tool_error)?;
        let has_changes = params.project.is_some() || params.title.is_some() || params.description.is_some() || priority.is_some() || params.labels.is_some();
        self.store.with_connection(|conn| {
            fetch_item(conn, &params.id)?;
            if has_changes {
                update_item(conn, &params.id, &ItemUpdate { project: params.project, title: params.title, description: params.description, priority, labels: params.labels, ..ItemUpdate::default() })?;
                insert_event(conn, &params.id, MCP_ACTOR, "updated", None, None)?;
            }
            pretty_json(&fetch_item(conn, &params.id)?)
        }).map_err(tool_error)
    }
}

#[tool(tool_box)]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation { name: "ai-board".to_owned(), version: env!("CARGO_PKG_VERSION").to_owned() },
            instructions: Some("Persistent backlog manager for AI agents.".to_owned()),
            ..Default::default()
        }
    }
}

fn detect_project() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|path| path.file_name().and_then(|name| name.to_str()).map(str::to_owned))
        .unwrap_or_else(|| "default".to_owned())
}
fn repo_path() -> Result<String> { Ok(std::env::current_dir()?.display().to_string()) }
fn tool_error(error: anyhow::Error) -> String { error.to_string() }
fn pretty_json<T: Serialize>(value: &T) -> Result<String> { Ok(serde_json::to_string_pretty(value)?) }
fn ack(id: &str, action: &str) -> Result<String> { pretty_json(&json!({ "ok": true, "id": id, "action": action })) }
fn fetch_item(conn: &rusqlite::Connection, id: &str) -> Result<WorkItem> { get_item(conn, id)?.ok_or_else(|| anyhow!("work item not found: {id}")) }
fn current_actor(item: &WorkItem) -> String { item.assignee.clone().unwrap_or_else(|| MCP_ACTOR.to_owned()) }
fn push_aid_task(item: &mut WorkItem, task_id: &str) { if !item.aid_task_ids.iter().any(|existing| existing == task_id) { item.aid_task_ids.push(task_id.to_owned()); } }
fn normalize_assignee(assignee: Option<String>) -> Result<Option<String>> {
    assignee.map(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() { bail!("assignee cannot be empty"); }
        Ok(if trimmed.starts_with("agent:") { trimmed.to_owned() } else { format!("agent:{trimmed}") })
    }).transpose()
}
fn parse_status(value: Option<String>) -> Result<Option<Status>> { value.map(|value| parse_status_value(&value)).transpose() }
fn parse_priority(value: Option<String>) -> Result<Option<Priority>> { value.map(|value| parse_priority_value(&value)).transpose() }
fn parse_status_value(value: &str) -> Result<Status> {
    match value.trim().to_ascii_lowercase().as_str() {
        "backlog" => Ok(Status::Backlog), "ready" => Ok(Status::Ready), "active" => Ok(Status::Active),
        "review" => Ok(Status::Review), "done" => Ok(Status::Done), "blocked" => Ok(Status::Blocked),
        "rejected" => Ok(Status::Rejected), _ => bail!("invalid status: {value}"),
    }
}
fn parse_priority_value(value: &str) -> Result<Priority> {
    match value.trim().to_ascii_lowercase().as_str() {
        "low" => Ok(Priority::Low), "medium" => Ok(Priority::Medium),
        "high" => Ok(Priority::High), "critical" => Ok(Priority::Critical),
        _ => bail!("invalid priority: {value}"),
    }
}
