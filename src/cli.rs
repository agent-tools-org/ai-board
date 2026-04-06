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
    Item {
        #[command(subcommand)]
        command: ItemCommand,
    },
    Mcp,
    Next(NextArgs),
    Serve(ServeArgs),
    Init,
}

#[derive(Debug, Subcommand)]
pub enum ItemCommand {
    Create {
        title: String,
        #[arg(long, short)]
        project: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long, default_value = "medium")]
        priority: Priority,
        #[arg(long)]
        label: Vec<String>,
        #[arg(long)]
        parent: Option<String>,
        #[arg(long = "depends-on")]
        depends_on: Vec<String>,
        #[arg(long)]
        assignee: Option<String>,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        verify: Option<String>,
        #[arg(long)]
        estimate: Option<String>,
        #[arg(long = "due-date")]
        due_date: Option<String>,
        #[arg(long)]
        auto_dispatch: bool,
    },
    List {
        #[arg(long, short)]
        project: Option<String>,
        #[arg(long)]
        status: Option<Status>,
        #[arg(long)]
        priority: Option<Priority>,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        assignee: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    Show {
        id: String,
    },
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
    Update {
        id: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        priority: Option<Priority>,
        #[arg(long)]
        status: Option<Status>,
        #[arg(long)]
        label: Vec<String>,
        #[arg(long)]
        assignee: Option<String>,
        #[arg(long)]
        position: Option<f64>,
    },
    Delete {
        id: String,
    },
}

#[derive(Debug, clap::Args)]
pub struct NextArgs {
    #[arg(long, short)]
    pub project: Option<String>,
    #[arg(long)]
    pub label: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct ServeArgs {
    #[arg(long, default_value_t = 3100)]
    pub port: u16,
}
