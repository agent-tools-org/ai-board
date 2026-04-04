# ai-board

AI engineering backlog manager. Persistent task board where AI agents are the primary users and humans manage via web dashboard.

## Architecture

- **Backend**: Rust + Axum + SQLite
- **Frontend**: Embedded SPA (vanilla JS + CSS, no build step)
- **API**: REST + SSE for real-time
- **Single binary**: web assets embedded via rust-embed

## Development

```bash
cargo run -- serve              # start server on :3100
cargo run -- item create "Fix parser" --priority high
cargo run -- next               # show highest-priority ready item
```

## Code Style

- File size limit: 300 lines per file
- Function limit: 50 lines
- No `unwrap()` in production code
- No `features = ["full"]` for dependencies
- All public functions must have tests

## Database

SQLite at `~/.ai-board/db.sqlite3`. WAL mode for concurrency.

## Design

See DESIGN.md for full architecture and API specification.
