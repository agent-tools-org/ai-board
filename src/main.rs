// ai-board CLI entrypoint and command dispatch.
// Wires clap parsing to the SQLite store and terminal rendering.

mod api;
mod cli;
mod mcp;
mod next;
mod render;
mod store;
mod types;
mod web;

use std::{env, fs};

use anyhow::{Context, Result, anyhow};
use chrono::Local;
use clap::Parser;

use crate::cli::{Cli, Command, CreateArgs, ItemCommand, ListArgs, NextArgs, ServeArgs, UpdateArgs};
use crate::next::Pick;
use crate::render::{print_artifacts, print_item, print_list};
use crate::store::{ItemFilter, ItemUpdate, Store};
use crate::types::{Artifact, ArtifactType, Status, WorkItem};

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Item { command } => handle_item(command),
        Command::Create(args) => handle_create_args(args),
        Command::List(args) => handle_list_args(args),
        Command::Show { id } => handle_show(&id),
        Command::Update(args) => handle_update_args(args),
        Command::Delete { id } => handle_delete(&id),
        Command::Mcp => handle_mcp(),
        Command::Next(args) => handle_next(args),
        Command::Serve(args) => handle_serve(args),
        Command::Init => handle_init(),
    }
}

fn handle_item(command: ItemCommand) -> Result<()> {
    match command {
        ItemCommand::Create(args) => handle_create_args(args),
        ItemCommand::List(args) => handle_list_args(args),
        ItemCommand::Show { id } => handle_show(&id),
        ItemCommand::Attach { item_id, artifact_type, title, path, content, status } =>
            handle_attach(&item_id, artifact_type, title, path, content, status),
        ItemCommand::Artifacts { item_id } => handle_artifacts(&item_id),
        ItemCommand::Update(args) => handle_update_args(args),
        ItemCommand::Delete { id } => handle_delete(&id),
    }
}

fn handle_create_args(args: CreateArgs) -> Result<()> {
    let title = args.title().ok_or_else(|| anyhow!("title is required — provide as positional arg or --title"))?.to_owned();
    let project = create_project(args.project)?;
    let store = open_store()?;
    let now = Local::now();
    let item = WorkItem {
        id: crate::store::gen_id(),
        project,
        repo_path: repo_path()?,
        title,
        description: args.description.unwrap_or_default(),
        status: Status::Backlog,
        priority: args.priority,
        position: now.timestamp_millis() as f64,
        labels: args.label,
        parent_id: args.parent,
        depends_on: args.depends_on,
        aid_task_ids: Vec::new(),
        aid_agent: args.agent,
        aid_verify: args.verify,
        estimate: args.estimate,
        assignee: args.assignee,
        created_by: "human:cli".to_owned(),
        requires_approval: false,
        auto_dispatch: args.auto_dispatch,
        created_at: now,
        updated_at: now,
        started_at: None,
        completed_at: None,
        due_date: args.due_date,
    };
    crate::store::insert_item(&store.connection(), &item)?;
    crate::store::insert_event(&store.connection(), &item.id, "human:cli", "created", Some("Created via CLI"), None)?;
    println!("Created {}", item.id);
    Ok(())
}

fn handle_list_args(args: ListArgs) -> Result<()> {
    let store = open_store()?;
    let project = if args.all { None } else { Some(detect_project(args.project)?) };
    let filter = ItemFilter {
        status: args.status,
        priority: args.priority,
        label: args.label,
        assignee: args.assignee,
        project: project.clone(),
        // One extra row turns "cut off by --limit" from a guess into an observation.
        limit: Some(args.limit.saturating_add(1)),
        ..ItemFilter::default()
    };
    let mut items = crate::store::list_items(&store.connection(), &filter)?;
    let truncated = items.len() > args.limit;
    items.truncate(args.limit);
    print_list(&items, project.as_deref(), truncated);
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

fn handle_update_args(args: UpdateArgs) -> Result<()> {
    let project = args.project.map(validate_project).transpose()?;
    let store = open_store()?;
    let id = &args.id;
    let _ = load_item(&store.connection(), id)?;
    crate::store::update_item(
        &store.connection(),
        id,
        &ItemUpdate {
            project,
            repo_path: None,
            title: args.title,
            description: args.description,
            priority: args.priority,
            position: args.position,
            labels: (!args.label.is_empty()).then_some(args.label),
            parent_id: None,
            depends_on: None,
            aid_task_ids: None,
            aid_agent: None,
            aid_verify: None,
            estimate: None,
            assignee: args.assignee.map(Some),
            requires_approval: None,
            auto_dispatch: None,
            due_date: None,
        },
    )?;
    if let Some(status) = args.status { crate::store::update_item_status(&store.connection(), id, status, args.force)?; }
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
    match crate::next::pick(&store, &project, args.label.as_deref())? {
        Pick::Found(item, events) => print_item(&item, &events),
        Pick::Empty { elsewhere } => {
            // "nothing pickable", not "no ready items": an item can be ready and still be
            // waiting on a dependency, and a label filter narrows the search too.
            let scope = match args.label.as_deref() {
                Some(label) => format!("project '{project}' with label '{label}'"),
                None => format!("project '{project}'"),
            };
            println!("Nothing ready to pick up in {scope}");
            if !elsewhere.is_empty() {
                println!("Ready and unblocked in: {} (use --project <name>)", elsewhere.join(", "));
            }
        }
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

fn create_project(explicit: Option<String>) -> Result<String> {
    detect_project(explicit.map(validate_project).transpose()?)
}

/// `-p` is `--project`; `-p high` used to file the item under a project literally named
/// "high" and silently lose the intended priority. Only the priority words are refused —
/// `review` or `done` are odd project names but nothing on the command line confuses them.
fn validate_project(project: String) -> Result<String> {
    const PRIORITIES: [&str; 4] = ["critical", "high", "medium", "low"];
    if PRIORITIES.contains(&project.to_ascii_lowercase().as_str()) {
        return Err(anyhow!(
            "-p/--project got '{project}', which is a priority, not a project. Priority is `--priority {project}` — it has no short flag."
        ));
    }
    Ok(project)
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
