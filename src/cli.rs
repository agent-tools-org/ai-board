// Clap CLI definitions for ai-board commands.
// Reuses crate::types enums so CLI and store share one domain model.

use clap::{Parser, Subcommand};

use crate::types::{ArtifactType, Priority, Status};

#[derive(Debug, Parser)]
#[command(name = "ai-board", version, about = "AI engineering backlog manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Work item subcommands (ai-board item create/list/show/...)
    Item {
        #[command(subcommand)]
        command: ItemCommand,
    },
    /// Create a new work item (shortcut for `item create`)
    #[command(alias = "c", alias = "add")]
    Create(CreateArgs),
    /// List work items (shortcut for `item list`)
    #[command(alias = "ls")]
    List(ListArgs),
    /// Show a work item (shortcut for `item show`)
    #[command(alias = "s")]
    Show { id: String },
    /// Update a work item (shortcut for `item update`)
    #[command(alias = "up")]
    Update(UpdateArgs),
    /// Delete a work item (shortcut for `item delete`)
    #[command(alias = "rm")]
    Delete { id: String },
    Mcp,
    Next(NextArgs),
    Serve(ServeArgs),
    Init,
}

#[derive(Debug, Subcommand)]
pub enum ItemCommand {
    #[command(alias = "c", alias = "add")]
    Create(CreateArgs),
    #[command(alias = "ls")]
    List(ListArgs),
    #[command(alias = "s")]
    Show { id: String },
    Attach {
        item_id: String,
        #[arg(long, short = 't')]
        artifact_type: ArtifactType,
        #[arg(long)]
        title: String,
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        content: Option<String>,
        #[arg(long, default_value = "final")]
        status: String,
    },
    Artifacts {
        item_id: String,
    },
    #[command(alias = "up")]
    Update(UpdateArgs),
    #[command(alias = "rm")]
    Delete { id: String },
}

#[derive(Debug, clap::Args)]
pub struct CreateArgs {
    /// Title (positional or --title flag)
    #[arg(value_name = "TITLE", default_value = "")]
    pub title_positional: String,
    #[arg(long = "title", short = 'T')]
    pub title_flag: Option<String>,
    #[arg(long, short)]
    pub project: Option<String>,
    #[arg(long, short = 'd')]
    pub description: Option<String>,
    #[arg(long, default_value = "medium")]
    pub priority: Priority,
    #[arg(long, short = 'l')]
    pub label: Vec<String>,
    #[arg(long)]
    pub parent: Option<String>,
    #[arg(long = "depends-on")]
    pub depends_on: Vec<String>,
    #[arg(long, short = 'a')]
    pub assignee: Option<String>,
    #[arg(long)]
    pub agent: Option<String>,
    #[arg(long)]
    pub verify: Option<String>,
    #[arg(long)]
    pub estimate: Option<String>,
    #[arg(long = "due-date")]
    pub due_date: Option<String>,
    #[arg(long)]
    pub auto_dispatch: bool,
}

impl CreateArgs {
    pub fn title(&self) -> Option<&str> {
        if let Some(ref t) = self.title_flag {
            Some(t.as_str())
        } else if !self.title_positional.is_empty() {
            Some(self.title_positional.as_str())
        } else {
            None
        }
    }
}

#[derive(Debug, clap::Args)]
pub struct ListArgs {
    #[arg(long, short)]
    pub project: Option<String>,
    /// List items from every project instead of the current one
    #[arg(long, short = 'A', conflicts_with = "project")]
    pub all: bool,
    #[arg(long, short = 's')]
    pub status: Option<Status>,
    #[arg(long)]
    pub priority: Option<Priority>,
    #[arg(long, short = 'l')]
    pub label: Option<String>,
    #[arg(long, short = 'a')]
    pub assignee: Option<String>,
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
}

#[derive(Debug, clap::Args)]
pub struct UpdateArgs {
    pub id: String,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long, short = 'd')]
    pub description: Option<String>,
    #[arg(long)]
    pub priority: Option<Priority>,
    #[arg(long, short = 's')]
    pub status: Option<Status>,
    #[arg(long, short = 'l')]
    pub label: Vec<String>,
    #[arg(long, short = 'a')]
    pub assignee: Option<String>,
    #[arg(long)]
    pub position: Option<f64>,
    #[arg(long, short)]
    pub force: bool,
}

#[derive(Debug, clap::Args)]
pub struct NextArgs {
    #[arg(long, short)]
    pub project: Option<String>,
    #[arg(long, short = 'l')]
    pub label: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct ServeArgs {
    #[arg(long, default_value_t = 3100)]
    pub port: u16,
}
