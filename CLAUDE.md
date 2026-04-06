# ai-board

AI engineering backlog manager. Persistent task board where AI agents are the primary users and humans manage via web dashboard.

## Architecture

- **Backend**: Rust + Axum + SQLite (centralized at `~/.ai-board/db.sqlite3`)
- **Frontend**: Embedded SPA (vanilla JS + CSS, no build step)
- **API**: REST + SSE for real-time
- **MCP**: stdio transport for Claude Code integration
- **Single binary**: web assets embedded via rust-embed

## Development

```bash
cargo run -- serve              # start server on :3100 (dashboard + API)
cargo run -- item create "Fix parser" --priority high --project myapp
cargo run -- item list --project myapp
cargo run -- next --project myapp
cargo run -- item attach wi-xxxx -t design_doc --title "Design" --content "# Plan\n..."
cargo run -- item artifacts wi-xxxx
cargo run -- item update wi-xxxx --status active        # gate-checked
cargo run -- item update wi-xxxx --status done --force   # bypass gate
cargo run -- mcp                # start MCP server (stdio)
```

## Key Concepts

### Project Scoping
Items are organized by `project` (auto-detected from current directory name, or `--project`/`-p`). Centralized DB at `~/.ai-board/db.sqlite3` holds all projects.

### Workflow Gates
Status transitions enforce artifact requirements:
- `ready → active`: requires `design_doc` or `investigation` artifact with status `final`
- `active → review/done`: requires `audit_report` artifact
- Bypass with `--force` (CLI) or `force` param (API)

### Artifacts
Documents attached to work items: `design_doc`, `investigation`, `audit_report`. Each has title, optional content (markdown), optional file path, and status (`draft`/`final`).

## Code Style

- File size limit: 300 lines per file
- Function limit: 50 lines
- No `unwrap()` in production code
- No `features = ["full"]` for dependencies

## Design

See DESIGN.md for full architecture and API specification.
