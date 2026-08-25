# ai-board

Persistent backlog manager for AI-driven engineering workflows. AI agents are the primary users (API-first), humans manage via web dashboard.

```
Human Admin                AI Agent (Claude Code, etc.)
    |                              |
    |  [web dashboard]             |  [REST API / MCP]
    v                              v
 +-----------------------------------------+
 |              ai-board                    |
 |  backlog · priorities · dependencies    |
 |  workflow gates · progress tracking     |
 +-----------------------------------------+
```

## Features

- **Work item board** — Kanban columns with drag-and-drop, priority queue, dependency tracking
- **Agent API** — REST endpoints for AI agents to claim, complete, and report on work
- **MCP server** — Native tool integration for Claude Code and other MCP-capable agents
- **Web dashboard** — Real-time updates via SSE, project filtering, responsive layout
- **Workflow gates** — Enforce artifacts (design docs, investigations, audit reports) before status transitions
- **Single binary** — Rust + SQLite, web assets embedded via rust-embed, zero external dependencies

## Install

```bash
cargo install ai-board
```

Or build from source:

```bash
git clone https://github.com/agent-tools-org/ai-board.git
cd ai-board
cargo build --release
cp target/release/ai-board ~/.cargo/bin/
```

## Quick Start

```bash
# Initialize (creates ~/.ai-board/db.sqlite3)
ai-board init

# Create work items
ai-board item create "Fix parser edge case" --priority high --label bug
ai-board item create "Add retry logic" --priority medium

# See what's next
ai-board next

# Attach a design doc (workflow gate)
ai-board item attach wi-xxxx -t design_doc --title "Design" --content "# Plan" --status final

# Update status (gate-checked)
ai-board item update wi-xxxx --status active

# Start web dashboard
ai-board serve
# Open http://localhost:3100
```

## CLI Reference

```bash
ai-board init                              # Initialize board
ai-board serve [--port 3100]               # Start web dashboard + API
ai-board mcp                               # Start MCP server (stdio)

# Items
ai-board item create "Title" [options]     # Create work item
ai-board item list [--status ready]        # List items (filterable, current project)
ai-board item list --all                   # List items across every project
ai-board item show <id>                    # Show item details + history
ai-board item update <id> [--status ...]   # Update item fields
ai-board item delete <id>                  # Delete item

# Artifacts & Gates
ai-board item attach <id> -t design_doc --title "Doc" --content "..." --status final
ai-board item artifacts <id>               # List artifacts

# Next item
ai-board next [--project myapp]            # Highest-priority ready item
```

### Create Options

| Flag | Description |
|------|-------------|
| `--project`, `-p` | Project name (default: current dir name) |
| `--priority` | `low`, `medium`, `high`, `critical` — long flag only, `-p` is `--project` |
| `--label` | Labels (repeatable) |
| `--parent` | Parent work item ID |
| `--depends-on` | Dependency IDs (repeatable) |
| `--assignee` | Assign to agent or human |
| `--agent` | Preferred AI agent for dispatch |
| `--verify` | Verification command |
| `--estimate` | Effort estimate (S/M/L/XL) |
| `--due-date` | Due date (ISO format) |

## Project Scoping

All projects share a centralized database at `~/.ai-board/db.sqlite3`. Items are scoped by `project`, which is auto-detected from the current directory name.

```bash
cd ~/projects/myapp
ai-board item list                # lists items for "myapp"
ai-board item list -p other-app   # lists items for "other-app"
ai-board item list --all          # every project, with a PROJECT column
```

Every listing prints the scope it queried in its header, so an item filed under another
project is never missing without an explanation:

```
project: myapp · 12 item(s) · -A/--all lists every project
```

`next` is scoped the same way and names the projects that do hold ready work when the
current one has none. Because scope follows the directory, file a hand-off item with the
`-p` the next session will run under and verify it with `ai-board next -p <project>` —
`show` looks items up by id and is project-blind.

## Workflow Gates

Status transitions can enforce artifact requirements:

| Transition | Required Artifact | Bypass |
|------------|-------------------|--------|
| `ready -> active` | `design_doc` or `investigation` (status: `final`) | `--force` |
| `active -> review/done` | `audit_report` | `--force` |

## MCP Integration

Add to your Claude Code MCP config (`.claude/settings.json`):

```json
{
  "mcpServers": {
    "ai-board": {
      "command": "ai-board",
      "args": ["mcp"]
    }
  }
}
```

Available tools: `board_next`, `board_list`, `board_show`, `board_create`, `board_claim`, `board_complete`, `board_block`, `board_submit`, `board_note`, `board_update`.

## REST API

### Agent Endpoints

```
GET    /api/agent/next                    # Next ready item
POST   /api/agent/items/{id}/claim        # Claim item (-> active)
POST   /api/agent/items/{id}/complete     # Mark done
POST   /api/agent/items/{id}/block        # Report blocker
POST   /api/agent/items/{id}/submit       # Submit for review
POST   /api/agent/items/{id}/note         # Add progress note
GET    /api/agent/context/{id}            # Item + deps + history
```

### Dashboard Endpoints

```
GET    /api/items                         # List items
POST   /api/items                         # Create item
PATCH  /api/items/{id}                    # Update item
DELETE /api/items/{id}                    # Delete item
PATCH  /api/items/reorder                 # Batch reorder
GET    /api/items/{id}/events             # Item history
GET    /api/items/{id}/artifacts          # Item artifacts
POST   /api/items/{id}/artifacts          # Add artifact
GET    /api/stream                        # SSE event stream
```

## Architecture

| Layer | Technology |
|-------|-----------|
| Language | Rust |
| Web framework | Axum |
| Database | SQLite (WAL mode) |
| Frontend | Vanilla JS + CSS (embedded) |
| Real-time | Server-Sent Events (SSE) |
| AI integration | MCP (stdio transport) |

## License

MIT
