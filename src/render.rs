// Terminal rendering helpers for ai-board CLI output.
// Formats fixed-width tables and detailed item views from shared types.

use chrono::{DateTime, Duration, Local};

use crate::types::{Artifact, ArtifactType, Event, Priority, Status, WorkItem};

const RESET: &str = "\x1b[0m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const WHITE: &str = "\x1b[37m";
const DIM: &str = "\x1b[2m";

/// Prints a listing. `project` is the scope that was queried; `None` means every project,
/// which adds a PROJECT column so rows stay identifiable. Every way a row can be missing
/// from the output — project scope, `--limit` — is named in the header.
pub fn print_list(items: &[WorkItem], project: Option<&str>, truncated: bool) {
    let scope = match project {
        Some(project) => format!("project: {project}"),
        None => "all projects".to_owned(),
    };
    let mut notes = vec![format!("{} item(s)", items.len())];
    if truncated {
        notes.push("cut off by --limit".to_owned());
    }
    if project.is_some() {
        notes.push("-A/--all lists every project".to_owned());
    }
    println!("{DIM}{scope} · {}{RESET}", notes.join(" · "));
    let with_project = project.is_none();
    println!("{}", list_header(with_project));
    for item in items {
        println!("{}", list_row(item, with_project));
    }
}

fn list_header(with_project: bool) -> String {
    let mut columns = vec![pad("ID", 8), pad("PRI", 8), pad("STATUS", 8)];
    if with_project {
        columns.push(pad("PROJECT", 16));
    }
    columns.push(pad("TITLE", 24));
    columns.push(pad("ASSIGNEE", 12));
    columns.push("UPDATED".to_owned());
    columns.join(" ")
}

fn list_row(item: &WorkItem, with_project: bool) -> String {
    let mut columns = vec![
        pad(&item.id, 8),
        color_priority(&pad(priority_text(&item.priority), 8), &item.priority),
        pad(status_text(&item.status), 8),
    ];
    if with_project {
        columns.push(pad(&item.project, 16));
    }
    columns.push(pad(&item.title, 24));
    columns.push(pad(item.assignee.as_deref().unwrap_or("—"), 12));
    columns.push(relative_time(item.updated_at));
    columns.join(" ")
}

pub fn print_item(item: &WorkItem, events: &[Event]) {
    println!("ID: {}", item.id);
    println!("Title: {}", item.title);
    println!("Project: {}", item.project);
    println!("Repo: {}", item.repo_path);
    println!("Status: {}", status_text(&item.status));
    println!("Priority: {}", priority_text(&item.priority));
    println!("Assignee: {}", item.assignee.as_deref().unwrap_or("—"));
    println!("Labels: {}", joined_or_dash(&item.labels));
    println!("Depends on: {}", joined_or_dash(&item.depends_on));
    println!("Agent: {}", item.aid_agent.as_deref().unwrap_or("—"));
    println!("Verify: {}", item.aid_verify.as_deref().unwrap_or("—"));
    println!("Estimate: {}", item.estimate.as_deref().unwrap_or("—"));
    println!("Due date: {}", item.due_date.as_deref().unwrap_or("—"));
    println!("Updated: {}", item.updated_at.to_rfc3339());
    println!();
    println!("Description:");
    println!("{}", if item.description.is_empty() { "—" } else { &item.description });
    println!();
    println!("Events:");
    if events.is_empty() {
        println!("—");
        return;
    }
    for event in events {
        println!(
            "{}  {}  {}  {}",
            event.created_at.to_rfc3339(),
            event.actor,
            event.action,
            event.detail.as_deref().unwrap_or("—"),
        );
    }
}

pub fn print_artifacts(artifacts: &[Artifact]) {
    println!(
        "{} {} {} {} UPDATED",
        pad("ID", 8),
        pad("TYPE", 14),
        pad("STATUS", 8),
        pad("TITLE", 28),
    );
    for artifact in artifacts {
        println!(
            "{} {} {} {} {}",
            pad(&artifact.id, 8),
            pad(artifact_type_text(&artifact.artifact_type), 14),
            pad(&artifact.status, 8),
            pad(&artifact.title, 28),
            relative_time(artifact.updated_at),
        );
    }
}

fn joined_or_dash(values: &[String]) -> String {
    if values.is_empty() { "—".to_owned() } else { values.join(", ") }
}

fn priority_text(priority: &Priority) -> &'static str {
    match priority {
        Priority::Critical => "critical",
        Priority::High => "high",
        Priority::Medium => "medium",
        Priority::Low => "low",
    }
}

fn status_text(status: &Status) -> &'static str {
    match status {
        Status::Backlog => "backlog",
        Status::Ready => "ready",
        Status::Active => "active",
        Status::Review => "review",
        Status::Done => "done",
        Status::Blocked => "blocked",
        Status::Rejected => "rejected",
    }
}

fn artifact_type_text(artifact_type: &ArtifactType) -> &'static str {
    match artifact_type {
        ArtifactType::DesignDoc => "design_doc",
        ArtifactType::Investigation => "investigation",
        ArtifactType::AuditReport => "audit_report",
    }
}

fn color_priority(value: &str, priority: &Priority) -> String {
    let color = match priority {
        Priority::Critical => RED,
        Priority::High => YELLOW,
        Priority::Medium => WHITE,
        Priority::Low => DIM,
    };
    format!("{color}{value}{RESET}")
}

fn pad(value: &str, width: usize) -> String {
    let short = if value.chars().count() > width {
        format!(
            "{}...",
            value.chars().take(width.saturating_sub(3)).collect::<String>()
        )
    } else {
        value.to_owned()
    };
    format!("{short:<width$}")
}

fn relative_time(value: DateTime<Local>) -> String {
    let delta = Local::now().signed_duration_since(value);
    if delta < Duration::minutes(1) {
        format!("{}s ago", delta.num_seconds().max(0))
    } else if delta < Duration::hours(1) {
        format!("{}m ago", delta.num_minutes())
    } else if delta < Duration::days(1) {
        format!("{}h ago", delta.num_hours())
    } else {
        format!("{}d ago", delta.num_days())
    }
}
