# ai-board: AI Engineering Backlog Manager

## Overview

`ai-board` is a persistent backlog/task-board for AI-driven engineering workflows. Unlike `aid` (which handles dispatch-level task execution), `ai-board` manages the **what** and **when** — the long-lived work items that span multiple `aid` dispatches, sessions, and days.

**Primary user**: AI agents (API-first)
**Secondary user**: Human administrators (web dashboard)

### Problem Statement

`aid` excels at "run this prompt on this agent now." But it lacks:
1. **Persistent backlog** — what needs to be done across sessions?
2. **Priority management** — what should be done next?
3. **Progress tracking** — how much of a larger goal is complete?
4. **Human oversight** — can a human reorder, approve, or veto work?
5. **Cross-session continuity** — an agent finishing work today should know what's queued for tomorrow

`ai-board` fills this gap as the planning/backlog layer that feeds into `aid` for execution.

### Relationship to aid

```
Human Admin                AI Agent (Claude Code, etc.)
    |                              |
    |  [web dashboard]             |  [REST API]
    v                              v
 +-----------------------------------------+
 |              ai-board                    |
 |  backlog · priorities · dependencies    |
 |  approval gates · progress tracking     |
 +-----------------------------------------+
            |                    ^
            | dispatch           | completion callback
            v                    |
 +-----------------------------------------+
 |                aid                       |
 |  agent dispatch · execution · worktrees |
 +-----------------------------------------+
            |
            v
     AI CLI Agents (codex, gemini, cursor, ...)
```

---

## Core Concepts

### Work Item (not "task" — avoids confusion with aid tasks)

A work item represents a unit of engineering work. It can be as small as "fix typo in README" or as large as "implement authentication system" (which would have sub-items).

```
WorkItem {
    id: "wi-a3f8"               // short hex ID
    title: String
    description: String          // markdown, can include acceptance criteria
    status: Status               // backlog → ready → active → review → done
    priority: Priority           // critical > high > medium > low
    position: f64                // fractional ordering within status column
    labels: Vec<String>          // ["bug", "frontend", "v2"]
    
    // Hierarchy
    parent_id: Option<WorkItemId>
    depends_on: Vec<WorkItemId>  // blocked until these are done
    blocks: Vec<WorkItemId>      // computed inverse
    
    // Execution link
    aid_task_ids: Vec<String>    // linked aid dispatches
    aid_agent: Option<String>    // preferred agent for dispatch
    
    // Effort
    estimate: Option<Estimate>   // t-shirt size or hours
    
    // Ownership
    assignee: Option<String>     // "agent:claude", "human:ming", or unassigned
    created_by: String
    
    // Gates
    requires_approval: bool      // human must approve before dispatch
    auto_dispatch: bool          // automatically dispatch when ready
    
    // Timestamps
    created_at, updated_at, started_at, completed_at
    due_date: Option<Date>
}
```

### Status Flow

```
backlog → ready → active → review → done
                    ↓         ↓
                  blocked   rejected → ready (re-queued)
```

| Status | Meaning |
|--------|---------|
| `backlog` | Known work, not yet prioritized |
| `ready` | Prioritized and unblocked, can be picked up |
| `active` | Currently being worked on (aid task dispatched) |
| `review` | Work complete, awaiting human review |
| `done` | Accepted and closed |
| `blocked` | Waiting on dependency or external input |
| `rejected` | Review failed, needs rework |

### Board (View)

A board is a saved view/filter over work items. Multiple boards can exist.

```
Board {
    id: "bd-01"
    name: String                 // "Sprint 12", "Bug Triage", "v2 Roadmap"
    filter: Filter               // status, labels, assignee, date range
    sort: SortOrder              // priority, position, created_at, due_date
    columns: Vec<StatusColumn>   // which statuses to show as columns
}
```

### Event Log

Every state change is recorded for auditability.

```
Event {
    id, timestamp
    item_id: WorkItemId
    actor: String                // "agent:claude", "human:ming", "system"
    action: Action               // created, status_changed, priority_changed, ...
    detail: String               // human-readable description
    metadata: Option<JSON>       // structured data (old/new values, aid task id, etc.)
}
```

---

## Architecture

### Tech Stack

| Layer | Choice | Rationale |
|-------|--------|-----------|
| Language | Rust | Consistent with aid, shared types possible |
| Web framework | Axum | Already proven in aid, async, performant |
| Database | SQLite | Portable, zero-ops, sufficient for single-team scale |
| Frontend | Vanilla JS + CSS (embedded) | No build step, lightweight, fast iteration |
| Real-time | SSE (Server-Sent Events) | Proven in aid, simpler than WebSocket for read-heavy updates |
| API | REST + JSON | Agent-friendly, simple, well-understood |

### Single Binary

Like `aid`, `ai-board` ships as a single binary with the web UI embedded via `rust-embed`. No separate frontend build step in production.

```bash
ai-board serve                    # start server on :3100
ai-board serve --port 3200        # custom port
ai-board item create "Fix parser" --priority high --label bug
ai-board item list --status ready
ai-board next                     # show highest-priority ready item
```

### Directory Structure

```
ai-board/
├── Cargo.toml
├── DESIGN.md                     # this file
├── CLAUDE.md                     # dev instructions
├── src/
│   ├── main.rs                   # CLI entry + server boot
│   ├── cli.rs                    # CLI argument parsing (clap)
│   ├── types.rs                  # WorkItem, Status, Priority, Artifact, Event
│   ├── render.rs                 # Terminal output formatting
│   ├── store/
│   │   ├── mod.rs                # SQLite connection pool (~/.ai-board/db.sqlite3)
│   │   ├── schema.rs             # table definitions + migrations
│   │   ├── items.rs              # work item CRUD + gate enforcement
│   │   ├── artifacts.rs          # artifact CRUD + gate checks
│   │   └── events.rs             # event log queries
│   ├── api/
│   │   ├── mod.rs                # Axum router setup
│   │   ├── items.rs              # /api/items endpoints
│   │   ├── artifacts.rs          # /api/items/{id}/artifacts endpoints
│   │   ├── agent.rs              # /api/agent/* (agent-facing endpoints)
│   │   └── sse.rs                # SSE event stream
│   ├── mcp/
│   │   ├── mod.rs                # MCP server setup (stdio transport)
│   │   ├── tools.rs              # Tool definitions + handlers
│   │   └── tests.rs              # MCP integration tests
│   └── web/
│       └── mod.rs                # Embedded asset serving (rust-embed)
├── web/
│   └── static/                   # Frontend SPA (embedded at compile time)
│       ├── index.html
│       ├── style.css
│       ├── app.js                # Main app logic
│       ├── artifacts.js          # Artifact UI + gate warnings
│       ├── list.js               # List view
│       ├── sse.js                # SSE client
│       └── utils.js              # Shared helpers
└── tests/
    └── api_tests.rs
```

---

## API Design

### Agent-Facing Endpoints (primary user)

These are what AI agents call to manage their work queue.

```
GET    /api/agent/next                    # get highest-priority ready item
GET    /api/agent/next?label=bug          # filtered
POST   /api/agent/items/{id}/claim        # claim item (status → active)
POST   /api/agent/items/{id}/complete     # mark done with summary
POST   /api/agent/items/{id}/block        # report blocker
POST   /api/agent/items/{id}/submit       # submit for review
POST   /api/agent/items/{id}/note         # add progress note
GET    /api/agent/context/{id}            # get item + deps + history for prompt injection
```

#### `GET /api/agent/next` Response

```json
{
    "item": {
        "id": "wi-a3f8",
        "title": "Fix parser edge case with empty input",
        "description": "The parser panics on empty string input...",
        "priority": "high",
        "labels": ["bug", "parser"],
        "depends_on": [],
        "context": "Related to wi-b2c1 (parser refactor)"
    },
    "dispatch_hint": {
        "agent": "codex",
        "scope": ["src/parser/"],
        "verify": "cargo test -p parser"
    }
}
```

#### `POST /api/agent/items/{id}/complete` Request

```json
{
    "summary": "Fixed empty input handling by adding early return",
    "aid_task_id": "t-1a2b",
    "evidence": "All parser tests pass, added 3 new test cases",
    "files_changed": ["src/parser/mod.rs", "tests/parser_test.rs"]
}
```

### Human-Facing Endpoints (dashboard)

```
# Items
GET    /api/items                         # list items (filterable)
GET    /api/items/{id}                    # get item details
POST   /api/items                         # create item
PATCH  /api/items/{id}                    # update item
DELETE /api/items/{id}                    # delete item
POST   /api/items/{id}/approve            # approve reviewed item → done
POST   /api/items/{id}/reject             # reject → ready (with feedback)
PATCH  /api/items/reorder                 # batch update positions

# Boards
GET    /api/boards                        # list boards
GET    /api/boards/{id}                   # get board with items
POST   /api/boards                        # create board
PATCH  /api/boards/{id}                   # update board

# Events
GET    /api/items/{id}/events             # item history
GET    /api/events                        # global event stream

# Stats
GET    /api/stats                         # throughput, velocity, agent performance

# SSE
GET    /api/stream                        # real-time updates
```

### Reorder Mechanism

Position uses fractional indexing (like Linear/Figma):
- Items have `position: f64` within each status column
- Moving item between two items: `new_pos = (above.pos + below.pos) / 2`
- Periodic rebalancing when positions get too close (< 0.001 apart)
- Drag-and-drop on dashboard sends `PATCH /api/items/reorder` with `[{id, position, status}]`

---

## Web Dashboard

### Design Principles

1. **Read-optimized** — human views the board, agents do the writing
2. **Real-time** — SSE pushes updates, no manual refresh needed
3. **Minimal** — not trying to be Jira; focused on priority queue + status tracking
4. **Mobile-friendly** — check status from phone

### Views

#### 1. Board View (default)
Kanban columns: `ready | active | review | done`
- Cards show: title, priority badge, labels, assignee, linked aid task status
- Drag-and-drop to reorder within column or move between columns
- Click card → detail panel (right sidebar or modal)

#### 2. List View
Table with sortable columns: priority, title, status, assignee, created, updated
- Bulk actions: select multiple → change status/priority/label
- Inline editing for quick changes

#### 3. Timeline View (future)
Gantt-style view for items with due dates and dependencies
- Show dependency arrows
- Highlight blocked items

### Detail Panel
- Full description (markdown rendered)
- Activity log (events)
- Linked aid tasks with status
- Sub-items (if parent)
- Edit controls (status, priority, labels, assignee)
- Approve/reject buttons (for items in review)

### Filters
- Status, priority, label, assignee
- Created/updated date range
- Has blockers / is blocked
- Free text search (title + description)

---

## aid Integration

### Option A: Loose Coupling (Recommended for v1)

`ai-board` and `aid` are independent binaries that communicate via:
1. **CLI bridge**: `ai-board` can invoke `aid run` to dispatch work
2. **Webhook/callback**: `aid` calls `ai-board` API on task completion
3. **Shared SQLite**: optional, read `aid`'s DB for task status display

```bash
# Agent workflow:
# 1. Query ai-board for next item
item=$(curl -s localhost:3100/api/agent/next | jq -r '.item.id')

# 2. Dispatch via aid
aid run codex "$(curl -s localhost:3100/api/agent/context/$item)" \
    --on-done "curl -X POST localhost:3100/api/agent/items/$item/complete"

# 3. ai-board auto-updates when aid task finishes
```

### Option B: Tight Coupling (future)

Shared Rust crate (`ai-common`) with types + store, allowing:
- `aid` to directly query `ai-board`'s backlog
- `ai-board` to embed `aid`'s dispatch engine
- Single unified database

### Hook Integration

`aid`'s hook system can trigger `ai-board` updates:

```json
// .aid/hooks.toml
[[hook]]
event = "task.completed"
command = "curl -X POST localhost:3100/api/agent/items/${ITEM_ID}/complete -d '{\"aid_task_id\": \"${TASK_ID}\"}'"
```

### Claude Code Integration

The orchestrator (Claude Code as 老张) can use both tools:

```bash
# Check what to work on
curl localhost:3100/api/agent/next

# Dispatch via aid
aid run codex "..." --on-done "curl -X POST localhost:3100/api/..."

# Or, ai-board CLI directly
ai-board next                     # what's next?
ai-board dispatch wi-a3f8         # auto-dispatch highest-priority item via aid
```

---

## Data Model (SQLite Schema)

```sql
CREATE TABLE items (
    id TEXT PRIMARY KEY,           -- "wi-xxxx"
    repo_path TEXT NOT NULL,       -- repo root path (scoping key)
    title TEXT NOT NULL,
    description TEXT DEFAULT '',
    status TEXT NOT NULL DEFAULT 'backlog',
    priority TEXT NOT NULL DEFAULT 'medium',
    position REAL NOT NULL DEFAULT 0.0,
    parent_id TEXT REFERENCES items(id),
    assignee TEXT,
    created_by TEXT NOT NULL,
    requires_approval INTEGER DEFAULT 0,
    auto_dispatch INTEGER DEFAULT 0,
    estimate TEXT,                  -- "S", "M", "L", "XL" or hours
    aid_agent TEXT,                 -- preferred agent
    aid_verify TEXT,                -- verify command
    due_date TEXT,                  -- ISO date
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT
);

CREATE TABLE item_labels (
    item_id TEXT REFERENCES items(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    PRIMARY KEY (item_id, label)
);

CREATE TABLE item_dependencies (
    item_id TEXT REFERENCES items(id) ON DELETE CASCADE,
    depends_on TEXT REFERENCES items(id) ON DELETE CASCADE,
    PRIMARY KEY (item_id, depends_on)
);

CREATE TABLE item_aid_tasks (
    item_id TEXT REFERENCES items(id) ON DELETE CASCADE,
    aid_task_id TEXT NOT NULL,
    status TEXT,                    -- mirrored from aid
    created_at TEXT NOT NULL,
    PRIMARY KEY (item_id, aid_task_id)
);

CREATE TABLE events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    item_id TEXT REFERENCES items(id) ON DELETE CASCADE,
    actor TEXT NOT NULL,            -- "agent:claude", "human:ming"
    action TEXT NOT NULL,           -- "created", "status_changed", etc.
    detail TEXT,
    metadata TEXT,                  -- JSON
    created_at TEXT NOT NULL
);

CREATE TABLE boards (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    filter TEXT,                    -- JSON filter spec
    sort_by TEXT DEFAULT 'position',
    columns TEXT,                   -- JSON array of statuses
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Indexes
CREATE INDEX idx_items_status ON items(status);
CREATE INDEX idx_items_priority ON items(priority);
CREATE INDEX idx_items_parent ON items(parent_id);
CREATE INDEX idx_items_position ON items(status, position);
CREATE INDEX idx_events_item ON events(item_id);
CREATE INDEX idx_events_created ON events(created_at);
CREATE INDEX idx_labels_label ON item_labels(label);
```

---

## Key Design Decisions

### Why not extend aid?

`aid` is an **execution engine** — it's fast, stateless per dispatch, and optimized for fire-and-forget. Adding long-lived backlog management would:
1. Bloat aid's already large binary (269 source files)
2. Mix concerns (execution vs. planning)
3. Force aid's task model to accommodate both short-lived dispatches and long-lived work items

Separate tools, single workflow. Unix philosophy.

### Why SQLite (not Postgres)?

- Single binary deployment, zero ops
- Sufficient for single-team / single-machine scale
- Consistent with aid's approach
- WAL mode handles concurrent reads well
- If scale demands it later, migrate to Postgres (schema is compatible)

### Why REST (not GraphQL)?

- AI agents work better with simple REST — predictable URLs, standard HTTP methods
- No query complexity for the client to manage
- GraphQL shines for complex frontend queries, but our frontend is simple
- REST is easier to call from shell scripts and `curl`

### Why Vanilla JS (not Leptos/React)?

- No build step — edit JS/CSS and recompile Rust to embed, zero frontend toolchain
- Lightweight — total frontend under 600 lines across 6 files
- Fast iteration — no WASM compile, no hydration complexity
- Sufficient — the dashboard is read-optimized, agents do the heavy work via API

### Fractional Indexing for Position

Instead of integer positions (which require renumbering on insert), use f64:
- Between items at 1.0 and 2.0 → insert at 1.5
- Rebalance when gap < 0.001
- Same approach used by Linear, Figma, and Notion

---

## Non-Goals (v1)

- Multi-user authentication (single-team, localhost)
- Notifications (email, Slack) — use aid's existing notification infra
- Time tracking
- Git integration (that's aid's job)
- Billing/invoicing
- Cloud hosting (local-first, maybe later)

---

## Resolved Decisions

1. **Frontend**: Vanilla JS + CSS (embedded via rust-embed). No build step, lightweight, fast iteration.
2. **MCP**: Yes — ai-board exposes its own MCP server. Claude Code (and other MCP-capable agents) can use board tools directly without curl. See MCP section below.
3. **Scope**: Centralized DB at `~/.ai-board/db.sqlite3` with `project` as the primary scoping dimension. Project auto-detected from directory name.
4. **Name**: `ai-board` — simple, clear, pairs with `aid`.

---

## MCP Server

ai-board runs as an MCP server so AI agents (Claude Code, etc.) can interact with the board as native tools — no curl or HTTP needed.

### Start Mode

```bash
ai-board mcp                     # start MCP server (stdio transport)
ai-board serve --mcp             # HTTP server + MCP server simultaneously
```

### Exposed Tools

| Tool | Description | Parameters |
|------|-------------|------------|
| `board_next` | Get highest-priority ready work item | `label?`, `assignee?` |
| `board_list` | List work items with filters | `status?`, `priority?`, `label?`, `limit?` |
| `board_show` | Get item details + history | `id` |
| `board_create` | Create a new work item | `title`, `description?`, `priority?`, `labels?`, `depends_on?` |
| `board_claim` | Claim a work item (→ active) | `id` |
| `board_complete` | Mark item done with evidence | `id`, `summary`, `aid_task_id?`, `files_changed?` |
| `board_block` | Report item blocked | `id`, `reason` |
| `board_submit` | Submit for human review | `id`, `summary?` |
| `board_note` | Add progress note | `id`, `note` |
| `board_update` | Update item fields | `id`, `title?`, `description?`, `priority?`, `labels?` |

### Claude Code Integration via MCP

In `.claude/settings.json`:

```json
{
    "mcpServers": {
        "ai-board": {
            "command": "ai-board",
            "args": ["mcp"],
            "cwd": "/path/to/repo"
        }
    }
}
```

Then Claude Code can directly call:
```
Use board_next to see what I should work on.
Use board_claim wi-a3f8 to claim the parser fix.
Use board_complete wi-a3f8 with summary "Fixed empty input handling".
```

No HTTP server needed for agent workflows — MCP handles it over stdio.

---

## Project Scoping

### Centralized Database

All projects share a single database at `~/.ai-board/db.sqlite3`. Items are scoped by the `project` field.

```bash
ai-board item list --project myapp     # list items for one project
ai-board item list                     # list items for auto-detected project (cwd basename)
ai-board serve                         # dashboard shows all projects, filterable
```

### Auto-Detection

When `--project` is not specified, the project name is auto-detected from the current directory name (basename of `$PWD`).

### Workflow Gates

Artifacts enforce process discipline on status transitions:

| Transition | Required Artifact | Bypass |
|------------|-------------------|--------|
| `ready → active` | `design_doc` or `investigation` (status: `final`) | `--force` |
| `active → review/done` | `audit_report` | `--force` |

```bash
ai-board item attach wi-xxxx -t design_doc --title "Design" --content "# Plan" --status final
ai-board item update wi-xxxx --status active          # succeeds (gate satisfied)
ai-board item update wi-xxxx --status done             # fails (missing audit_report)
ai-board item update wi-xxxx --status done --force     # bypasses gate
```

---

## Web Dashboard

### Architecture

- **Embedded SPA** — vanilla JS + CSS files in `web/static/`, compiled into the binary via `rust-embed`
- **No build step** — edit files and `cargo build` embeds them automatically
- **Modular JS** — split into `app.js`, `artifacts.js`, `list.js`, `sse.js`, `utils.js`

### Features

- **Board view**: Kanban columns (backlog/ready/active/review/done/blocked/rejected) with drag-and-drop
- **List view**: Sortable table with all fields
- **Detail panel**: Right sidebar with edit controls, event history, artifacts section
- **Filters**: Status, priority, label, project — all as toggle pills
- **Artifacts**: Create/view/delete artifacts, markdown content preview, gate warnings
- **SSE**: Real-time updates from `/api/stream`
- **Responsive**: Column stack on mobile

### Build

```bash
cargo build --release            # embeds web/static/ via rust-embed
cargo run -- serve               # start on :3100
```

---

## Updated Development Phases

### Phase 1: Core + CLI ✅
- [x] Project scaffold (Cargo.toml, types, store)
- [x] SQLite store with migrations
- [x] Work item CRUD
- [x] CLI commands: `item create/list/show/update/delete`, `next`
- [x] Event logging
- [x] Priority queue logic (next item selection)
- [x] Dependency tracking and blocked status

### Phase 2: REST API + MCP ✅
- [x] Axum server with item endpoints
- [x] Agent-facing endpoints (`/api/agent/*`)
- [x] SSE event stream
- [x] Reorder/position management
- [x] MCP server mode (stdio transport)
- [x] MCP tool definitions (board_next, board_claim, etc.)

### Phase 3: Web Dashboard ✅
- [x] Embedded SPA (vanilla JS + CSS, rust-embed)
- [x] Board (kanban) view with drag-and-drop
- [x] List view with sorting/filtering
- [x] Item detail panel (sidebar)
- [x] Real-time updates via SSE
- [x] Responsive/mobile layout
- [x] Project filter + badges

### Phase 4: Project Scoping + Workflow Gates ✅
- [x] Centralized DB (`~/.ai-board/db.sqlite3`)
- [x] `project` as first-class scoping dimension
- [x] Auto-detect project from directory name
- [x] Artifacts (design_doc, investigation, audit_report)
- [x] Gate enforcement on status transitions
- [x] Artifact content preview with markdown rendering
- [x] `--force` flag to bypass gates

### Phase 5: aid Integration (planned)
- [ ] `ai-board dispatch` command (invoke aid)
- [ ] Hook-based completion callback
- [ ] Aid task status mirroring
- [ ] Auto-dispatch for `auto_dispatch: true` items

### Phase 6: Advanced (future)
- [ ] Timeline/dependency visualization
- [ ] Velocity/throughput metrics
- [ ] Agent performance analytics
- [ ] Cross-project aggregation dashboard
- [ ] Approval workflow with notifications
- [ ] Import/export (markdown, CSV)
