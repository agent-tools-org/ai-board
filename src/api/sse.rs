// Server-sent event stream for recent item activity updates.
// Exports: stream() under /api/stream and polls SQLite every two seconds.

use std::{convert::Infallible, time::Duration};

use async_stream::stream;
use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use serde::Serialize;
use tokio::time::{MissedTickBehavior, interval};

use crate::{api::AppState, store::list_all_events};

pub async fn stream(
    State(store): State<AppState>,
) -> Sse<impl futures_core::Stream<Item = Result<Event, Infallible>>> {
    let mut last_seen = latest_event_id(&store).unwrap_or_default();
    let event_stream = stream! {
        let mut ticker = interval(Duration::from_secs(2));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let Ok(events) = store.with_connection(|conn| list_all_events(conn, Some(100))) else {
                continue;
            };
            let mut unseen = events
                .into_iter()
                .filter(|event| event.id > last_seen)
                .collect::<Vec<_>>();
            unseen.reverse();
            for event in unseen {
                last_seen = event.id;
                let payload = StreamEvent {
                    item_id: event.item_id,
                    action: event.action,
                    detail: event.detail,
                    timestamp: event.created_at.to_rfc3339(),
                };
                if let Ok(data) = serde_json::to_string(&payload) {
                    yield Ok(Event::default().data(data));
                }
            }
        }
    };
    Sse::new(event_stream).keep_alive(KeepAlive::default())
}

fn latest_event_id(store: &AppState) -> anyhow::Result<i64> {
    Ok(store
        .with_connection(|conn| list_all_events(conn, Some(1)))?
        .into_iter()
        .next()
        .map(|event| event.id)
        .unwrap_or_default())
}

#[derive(Serialize)]
struct StreamEvent {
    item_id: String,
    action: String,
    detail: Option<String>,
    timestamp: String,
}
