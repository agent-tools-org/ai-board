// MCP tool tests covering item creation, queue lookup, and lifecycle updates.
// Exports: module-local async tests for crate::mcp::tools::McpServer.
// Deps: crate::store::Store, rmcp Parameters extractor, tokio test runtime.

use std::{env::temp_dir, path::{Path, PathBuf}, sync::Arc};

use rmcp::handler::server::tool::Parameters;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    mcp::tools::{
        BoardBlockParams, BoardClaimParams, BoardCompleteParams, BoardCreateParams, BoardListParams,
        BoardNextParams, BoardNoteParams, BoardShowParams, BoardSubmitParams, BoardUpdateParams,
        McpServer,
    },
    store::{ItemUpdate, Store, update_item, update_item_status},
    types::{Event, Priority, Status, WorkItem},
};

#[derive(Deserialize)]
struct ItemWithEvents { item: WorkItem, events: Vec<Event> }

#[tokio::test]
async fn lists_shows_and_picks_ready_items_for_current_repo() {
    let path = test_db_path();
    let store = Arc::new(Store::open(&path).expect("open store"));
    let server = McpServer::new(store.clone());
    let created: WorkItem = serde_json::from_str(&server.board_create(Parameters(BoardCreateParams {
        project: None, title: "Triage queue".to_owned(),
        description: Some("Inspect queue order".to_owned()), priority: Some("high".to_owned()),
        labels: Some(vec!["ops".to_owned()]), depends_on: None,
    })).await.expect("create")).expect("decode created item");
    store.with_connection(|conn| update_item_status(conn, &created.id, Status::Ready)).expect("mark ready");

    let listed: Vec<WorkItem> = serde_json::from_str(&server.board_list(Parameters(BoardListParams {
        project: None, status: Some("ready".to_owned()), priority: Some("high".to_owned()),
        label: Some("ops".to_owned()), limit: Some(10),
    })).await.expect("list")).expect("decode items");
    let next: WorkItem = serde_json::from_str(&server.board_next(Parameters(BoardNextParams {
        project: None, label: Some("ops".to_owned()),
    })).await.expect("next")).expect("decode next item");
    let shown: ItemWithEvents = serde_json::from_str(&server.board_show(Parameters(BoardShowParams {
        id: created.id.clone(),
    })).await.expect("show")).expect("decode item details");

    assert_eq!(listed.len(), 1);
    assert_eq!(next.id, created.id);
    assert_eq!(shown.item.id, created.id);
    assert_eq!(shown.events[0].action, "created");

    drop(server);
    drop(store);
    cleanup_db_files(&path);
}

#[tokio::test]
async fn claims_updates_notes_and_completes_items() {
    let path = test_db_path();
    let store = Arc::new(Store::open(&path).expect("open store"));
    let server = McpServer::new(store.clone());
    let created: WorkItem = serde_json::from_str(&server.board_create(Parameters(BoardCreateParams {
        project: None, title: "Ship MCP".to_owned(), description: None, priority: None,
        labels: None, depends_on: None,
    })).await.expect("create")).expect("decode created item");

    let updated: WorkItem = serde_json::from_str(&server.board_update(Parameters(BoardUpdateParams {
        id: created.id.clone(), project: None, title: Some("Ship MCP server".to_owned()),
        description: None, priority: Some("critical".to_owned()),
        labels: Some(vec!["mcp".to_owned()]),
    })).await.expect("update")).expect("decode updated item");
    let claimed: WorkItem = serde_json::from_str(&server.board_claim(Parameters(BoardClaimParams {
        id: created.id.clone(), assignee: Some("alice".to_owned()),
    })).await.expect("claim")).expect("decode claimed item");
    server.board_note(Parameters(BoardNoteParams {
        id: created.id.clone(), note: "Connected rmcp tool handlers".to_owned(),
    })).await.expect("note");
    let completed: WorkItem = serde_json::from_str(&server.board_complete(Parameters(BoardCompleteParams {
        id: created.id.clone(), summary: "MCP server wired".to_owned(), aid_task_id: Some("t-123".to_owned()),
    })).await.expect("complete")).expect("decode completed item");
    let shown: ItemWithEvents = serde_json::from_str(&server.board_show(Parameters(BoardShowParams {
        id: created.id.clone(),
    })).await.expect("show")).expect("decode item details");

    assert_eq!(updated.priority, Priority::Critical);
    assert_eq!(claimed.assignee.as_deref(), Some("agent:alice"));
    assert_eq!(completed.status, Status::Done);
    assert!(completed.aid_task_ids.iter().any(|task_id| task_id == "t-123"));
    assert!(shown.events.iter().any(|event| event.action == "updated"));
    assert!(shown.events.iter().any(|event| event.action == "claimed"));
    assert!(shown.events.iter().any(|event| event.action == "note"));
    assert!(shown.events.iter().any(|event| event.action == "completed"));

    drop(server);
    drop(store);
    cleanup_db_files(&path);
}

#[tokio::test]
async fn submits_and_blocks_items_with_confirmation() {
    let path = test_db_path();
    let store = Arc::new(Store::open(&path).expect("open store"));
    let server = McpServer::new(store.clone());
    let review_item: WorkItem = serde_json::from_str(&server.board_create(Parameters(BoardCreateParams {
        project: None, title: "Needs review".to_owned(), description: None, priority: None,
        labels: None, depends_on: None,
    })).await.expect("create review item")).expect("decode review item");
    store.with_connection(|conn| update_item(conn, &review_item.id, &ItemUpdate {
        requires_approval: Some(true), ..ItemUpdate::default()
    })).expect("require approval");

    server.board_submit(Parameters(BoardSubmitParams {
        id: review_item.id.clone(), summary: Some("Ready for review".to_owned()),
    })).await.expect("submit");
    let review_details: ItemWithEvents = serde_json::from_str(&server.board_show(Parameters(BoardShowParams {
        id: review_item.id.clone(),
    })).await.expect("show review")).expect("decode review payload");

    let blocked_item: WorkItem = serde_json::from_str(&server.board_create(Parameters(BoardCreateParams {
        project: None, title: "Blocked task".to_owned(), description: None, priority: None,
        labels: None, depends_on: None,
    })).await.expect("create blocked item")).expect("decode blocked item");
    server.board_block(Parameters(BoardBlockParams {
        id: blocked_item.id.clone(), reason: "Waiting on schema".to_owned(),
    })).await.expect("block");
    let blocked_details: ItemWithEvents = serde_json::from_str(&server.board_show(Parameters(BoardShowParams {
        id: blocked_item.id.clone(),
    })).await.expect("show blocked")).expect("decode blocked payload");

    assert_eq!(review_details.item.status, Status::Review);
    assert_eq!(blocked_details.item.status, Status::Blocked);

    drop(server);
    drop(store);
    cleanup_db_files(&path);
}

fn test_db_path() -> PathBuf {
    temp_dir().join(format!("ai-board-mcp-{}.sqlite3", Uuid::new_v4()))
}

fn cleanup_db_files(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(path.with_extension("sqlite3-shm"));
    let _ = std::fs::remove_file(path.with_extension("sqlite3-wal"));
}
