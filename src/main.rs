// ai-board CLI entrypoint and command dispatch.
// Wires clap parsing to the SQLite store and terminal rendering.

mod api;
mod cli;
mod mcp;
mod render;
mod store;
mod types;
mod web;

use std::{env, fs};

use anyhow::{Context, Result, anyhow};
use chrono::Local;
use clap::Parser;

use crate::cli::{Cli, Command, ItemCommand, NextArgs, ServeArgs};
use crate::render::{print_artifacts, print_item, print_list};
use crate::store::{ItemFilter, ItemUpdate, Store};
use crate::types::{Artifact, ArtifactType, Priority, Status, WorkItem};

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Item { command } => handle_item(command),
        Command::Mcp => handle_mcp(),
        Command::Next(args) => handle_next(args),
        Command::Serve(args) => handle_serve(args),
        Command::Init => handle_init(),
    }
}

fn handle_item(command: ItemCommand) -> Result<()> {
    match command {
        ItemCommand::Create { title, project, description, priority, label, parent, depends_on, assignee, agent, verify, estimate, due_date, auto_dispatch } =>
            handle_create(title, project, description, priority, label, parent, depends_on, assignee, agent, verify, estimate, due_date, auto_dispatch),
        ItemCommand::List { project, status, priority, label, assignee, limit } =>
            handle_list(project, status, priority, label, assignee, limit),
        ItemCommand::Show { id } => handle_show(&id),
        ItemCommand::Attach { item_id, artifact_type, title, path, content, status } =>
            handle_attach(&item_id, artifact_type, title, path, content, status),
        ItemCommand::Artifacts { item_id } => handle_artifacts(&item_id),
        ItemCommand::Update { id, title, description, priority, status, label, assignee, position, force } =>
            handle_update(&id, title, description, priority, status, label, assignee, position, force),
        ItemCommand::Delete { id } => handle_delete(&id),
    }
}

fn handle_create(
    title: String,
    project: Option<String>,
    description: Option<String>,
    priority: Priority,
    label: Vec<String>,
    parent: Option<String>,
    depends_on: Vec<String>,
    assignee: Option<String>,
    agent: Option<String>,
    verify: Option<String>,
    estimate: Option<String>,
    due_date: Option<String>,
    auto_dispatch: bool,
) -> Result<()> {
    let store = open_store()?;
    let now = Local::now();
    let item = WorkItem {
        id: crate::store::gen_id(),
        project: detect_project(project)?,
        repo_path: repo_path()?,
        title,
        description: description.unwrap_or_default(),
        status: Status::Backlog,
        priority,
        position: now.timestamp_millis() as f64,
        labels: label,
        parent_id: parent,
        depends_on,
        aid_task_ids: Vec::new(),
        aid_agent: agent,
        aid_verify: verify,
        estimate,
        assignee,
        created_by: "human:cli".to_owned(),
        requires_approval: false,
        auto_dispatch,
        created_at: now,
        updated_at: now,
        started_at: None,
        completed_at: None,
        due_date,
    };
    crate::store::insert_item(&store.connection(), &item)?;
    crate::store::insert_event(&store.connection(), &item.id, "human:cli", "created", Some("Created via CLI"), None)?;
    println!("Created {}", item.id);
    Ok(())
}

fn handle_list(
    project: Option<String>,
    status: Option<Status>,
    priority: Option<Priority>,
    label: Option<String>,
    assignee: Option<String>,
    limit: usize,
) -> Result<()> {
    let store = open_store()?;
    let items = crate::store::list_items(&store.connection(), &item_filter(project, status, priority, label, assignee, limit)?)?;
    print_list(&items);
    Ok(())
}

fn handle_show(id: &str) -> Result<()> {
    let store = open_store()?;
    let item = load_item(&store.connection(), id)?;
    let events = crate::store::list_events(&store.connection(), id, Some(20))?;
    print_item(&item, &events);
    Ok(())
}

fn handle_attach(
    item_id: &str,
    artifact_type: ArtifactType,
    title: String,
    path: Option<String>,
    content: Option<String>,
    status: String,
) -> Result<()> {
    let store = open_store()?;
    let _ = load_item(&store.connection(), item_id)?;
    let now = Local::now();
    let artifact = Artifact {
        id: crate::store::gen_artifact_id(),
        item_id: item_id.to_owned(),
        artifact_type,
        title,
        path,
        content: content.unwrap_or_default(),
        status,
        created_by: "human:cli".to_owned(),
        created_at: now,
        updated_at: now,
    };
    crate::store::insert_artifact(&store.connection(), &artifact)?;
    println!("Attached {}", artifact.id);
    Ok(())
}

fn handle_artifacts(item_id: &str) -> Result<()> {
    let store = open_store()?;
    let _ = load_item(&store.connection(), item_id)?;
    print_artifacts(&crate::store::list_artifacts(&store.connection(), item_id)?);
    Ok(())
}

fn handle_update(
    id: &str,
    title: Option<String>,
    description: Option<String>,
    priority: Option<Priority>,
    status: Option<Status>,
    label: Vec<String>,
    assignee: Option<String>,
    position: Option<f64>,
    force: bool,
) -> Result<()> {
    let store = open_store()?;
    let _ = load_item(&store.connection(), id)?;
    crate::store::update_item(
        &store.connection(),
        id,
        &ItemUpdate {
            project: None,
            repo_path: None,
            title,
            description,
            priority,
            position,
            labels: (!label.is_empty()).then_some(label),
            parent_id: None,
            depends_on: None,
            aid_task_ids: None,
            aid_agent: None,
            aid_verify: None,
            estimate: None,
            assignee: assignee.map(Some),
            requires_approval: None,
            auto_dispatch: None,
            due_date: None,
        },
    )?;
    if let Some(status) = status { crate::store::update_item_status(&store.connection(), id, status, force)?; }
    crate::store::insert_event(&store.connection(), id, "human:cli", "updated", Some("Updated via CLI"), None)?;
    println!("Updated {id}");
    Ok(())
}

fn handle_delete(id: &str) -> Result<()> {
    let store = open_store()?;
    let _ = load_item(&store.connection(), id)?;
    crate::store::delete_item(&store.connection(), id)?;
    println!("Deleted {id}");
    Ok(())
}

fn handle_next(args: NextArgs) -> Result<()> {
    let store = open_store()?;
    let project = detect_project(args.project)?;
    match crate::store::next_item(&store.connection(), &project, args.label.as_deref())? {
        Some(item) => print_item(&item, &crate::store::list_events(&store.connection(), &item.id, Some(10))?),
        None => println!("No ready items"),
    }
    Ok(())
}

fn handle_serve(args: ServeArgs) -> Result<()> {
    let store = open_store()?;
    let state = std::sync::Arc::new(store);
    let app = crate::api::router(state);
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], args.port));
    println!("ai-board server running on http://{addr}");
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, app).await.map_err(|e| anyhow!(e))
        })
}

fn handle_init() -> Result<()> {
    let path = board_dir()?;
    fs::create_dir_all(&path)?;
    let _ = Store::open_default()?;
    println!("Initialized {}", path.display());
    Ok(())
}

fn handle_mcp() -> Result<()> {
    let store = std::sync::Arc::new(open_store()?);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(crate::mcp::run_server(store))
}

fn open_store() -> Result<Store> {
    Store::open_default().context("failed to open ai-board store")
}

fn load_item(conn: &rusqlite::Connection, id: &str) -> Result<WorkItem> {
    crate::store::get_item(conn, id)?.ok_or_else(|| anyhow!("work item not found: {id}"))
}

fn detect_project(explicit: Option<String>) -> Result<String> {
    if let Some(project) = explicit { return Ok(project); }
    let dir = env::current_dir()?;
    dir.file_name().and_then(|name| name.to_str()).map(str::to_owned).ok_or_else(|| anyhow!("cannot detect project name from current directory"))
}

fn repo_path() -> Result<String> {
    Ok(env::current_dir()?
        .into_os_string()
        .into_string()
        .map_err(|_| anyhow!("current directory is not valid UTF-8"))?)
}

fn board_dir() -> Result<std::path::PathBuf> {
    env::var_os("HOME").map(std::path::PathBuf::from).map(|path| path.join(".ai-board")).ok_or_else(|| anyhow!("HOME is not set"))
}

fn item_filter(
    project: Option<String>,
    status: Option<Status>,
    priority: Option<Priority>,
    label: Option<String>,
    assignee: Option<String>,
    limit: usize,
) -> Result<ItemFilter> {
    Ok(ItemFilter { status, priority, label, assignee, parent_id: None, project: Some(detect_project(project)?), repo_path: None, limit: Some(limit) })
}
