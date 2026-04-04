// ai-board CLI entrypoint and command dispatch.
// Wires clap parsing to the SQLite store and terminal rendering.

mod api;
mod cli;
mod render;
mod store;
mod types;

use std::{env, fs};

use anyhow::{Context, Result, anyhow};
use chrono::Local;
use clap::Parser;

use crate::cli::{Cli, Command, ItemCommand, NextArgs, ServeArgs};
use crate::render::{print_item, print_list};
use crate::store::{ItemFilter, ItemUpdate, Store};
use crate::types::{Priority, Status, WorkItem};

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Item { command } => handle_item(command),
        Command::Next(args) => handle_next(args),
        Command::Serve(args) => handle_serve(args),
        Command::Init => handle_init(),
    }
}

fn handle_item(command: ItemCommand) -> Result<()> {
    match command {
        ItemCommand::Create {
            title,
            description,
            priority,
            label,
            parent,
            depends_on,
            assignee,
            agent,
            verify,
            estimate,
            due_date,
            auto_dispatch,
        } => handle_create(
            title,
            description,
            priority,
            label,
            parent,
            depends_on,
            assignee,
            agent,
            verify,
            estimate,
            due_date,
            auto_dispatch,
        ),
        ItemCommand::List {
            status,
            priority,
            label,
            assignee,
            limit,
        } => handle_list(status, priority, label, assignee, limit),
        ItemCommand::Show { id } => handle_show(&id),
        ItemCommand::Update {
            id,
            title,
            description,
            priority,
            status,
            label,
            assignee,
            position,
        } => handle_update(
            &id,
            title,
            description,
            priority,
            status,
            label,
            assignee,
            position,
        ),
        ItemCommand::Delete { id } => handle_delete(&id),
    }
}

fn handle_create(
    title: String,
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
    crate::store::insert_item(store.connection(), &item)?;
    crate::store::insert_event(
        store.connection(),
        &item.id,
        "human:cli",
        "created",
        Some("Created via CLI"),
        None,
    )?;
    println!("Created {}", item.id);
    Ok(())
}

fn handle_list(
    status: Option<Status>,
    priority: Option<Priority>,
    label: Option<String>,
    assignee: Option<String>,
    limit: usize,
) -> Result<()> {
    let store = open_store()?;
    let items = crate::store::list_items(store.connection(), &item_filter(status, priority, label, assignee, limit)?)?;
    print_list(&items);
    Ok(())
}

fn handle_show(id: &str) -> Result<()> {
    let store = open_store()?;
    let item = load_item(store.connection(), id)?;
    let events = crate::store::list_events(store.connection(), id, Some(20))?;
    print_item(&item, &events);
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
) -> Result<()> {
    let store = open_store()?;
    let _ = load_item(store.connection(), id)?;
    crate::store::update_item(
        store.connection(),
        id,
        &ItemUpdate {
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
    if let Some(status) = status {
        crate::store::update_item_status(store.connection(), id, status)?;
    }
    crate::store::insert_event(
        store.connection(),
        id,
        "human:cli",
        "updated",
        Some("Updated via CLI"),
        None,
    )?;
    println!("Updated {id}");
    Ok(())
}

fn handle_delete(id: &str) -> Result<()> {
    let store = open_store()?;
    let _ = load_item(store.connection(), id)?;
    crate::store::delete_item(store.connection(), id)?;
    println!("Deleted {id}");
    Ok(())
}

fn handle_next(args: NextArgs) -> Result<()> {
    let store = open_store()?;
    match crate::store::next_item(store.connection(), &repo_path()?, args.label.as_deref())? {
        Some(item) => {
            let events = crate::store::list_events(store.connection(), &item.id, Some(10))?;
            print_item(&item, &events);
        }
        None => println!("No ready items"),
    }
    Ok(())
}

fn handle_serve(args: ServeArgs) -> Result<()> {
    println!("Starting server on :{}", args.port);
    Ok(())
}

fn handle_init() -> Result<()> {
    let path = env::current_dir()
        .context("failed to get current directory")?
        .join(".ai-board");
    fs::create_dir_all(&path)?;
    let _ = Store::open_default()?;
    println!("Initialized {}", path.display());
    Ok(())
}

fn open_store() -> Result<Store> {
    Store::open_default().context("failed to open .ai-board store")
}

fn load_item(conn: &rusqlite::Connection, id: &str) -> Result<WorkItem> {
    crate::store::get_item(conn, id)?.ok_or_else(|| anyhow!("work item not found: {id}"))
}

fn repo_path() -> Result<String> {
    Ok(env::current_dir()?
        .into_os_string()
        .into_string()
        .map_err(|_| anyhow!("current directory is not valid UTF-8"))?)
}

fn item_filter(
    status: Option<Status>,
    priority: Option<Priority>,
    label: Option<String>,
    assignee: Option<String>,
    limit: usize,
) -> Result<ItemFilter> {
    Ok(ItemFilter {
        status,
        priority,
        label,
        assignee,
        parent_id: None,
        repo_path: Some(repo_path()?),
        limit: Some(limit),
    })
}
