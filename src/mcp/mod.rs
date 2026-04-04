// MCP server wiring for ai-board stdio transport.
// Exports: run_server() and tool handlers from tools.rs.
// Deps: crate::store::Store, rmcp service runtime, tokio stdio.

pub mod tools;

#[cfg(test)]
mod tests;

use std::sync::Arc;

use anyhow::Result;
use rmcp::ServiceExt;

use crate::store::Store;

pub async fn run_server(store: Arc<Store>) -> Result<()> {
    let running = tools::McpServer::new(store)
        .serve((tokio::io::stdin(), tokio::io::stdout()))
        .await?;
    let _ = running.waiting().await?;
    Ok(())
}
