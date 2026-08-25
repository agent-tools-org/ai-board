// `ai-board next` selection, kept out of main.rs so the path that used to hang stays testable.
// Holds one store guard for the whole pick and explains an empty result with the same
// predicate the picker itself uses.

use anyhow::Result;

use crate::store::{Store, list_events, next_item, projects_with_next};
use crate::types::{Event, WorkItem};

/// What `next` found, or — having found nothing — where else work is actually pickable.
pub enum Pick {
    Found(Box<WorkItem>, Vec<Event>),
    Empty { elsewhere: Vec<String> },
}

pub fn pick(store: &Store, project: &str, label: Option<&str>) -> Result<Pick> {
    // One guard for the whole pick: `Store::connection` is not reentrant, and a match
    // scrutinee keeps its temporary alive for every arm.
    let conn = store.connection();
    match next_item(&conn, project, label)? {
        Some(item) => {
            let events = list_events(&conn, &item.id, Some(10))?;
            Ok(Pick::Found(Box::new(item), events))
        }
        None => {
            let elsewhere = projects_with_next(&conn, label)?
                .into_iter()
                .filter(|candidate| candidate != project)
                .collect();
            Ok(Pick::Empty { elsewhere })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{env::temp_dir, sync::Arc, sync::mpsc, thread, time::Duration};

    use chrono::Local;
    use uuid::Uuid;

    use super::{Pick, pick};
    use crate::store::{Store, insert_item};
    use crate::types::{Priority, Status, WorkItem};

    #[test]
    fn pick_returns_a_ready_item_instead_of_blocking_on_its_own_guard() {
        let path = temp_dir().join(format!("ai-board-next-{}.sqlite3", Uuid::new_v4()));
        let store = Arc::new(Store::open(&path).expect("open store"));
        store
            .with_connection(|conn| insert_item(conn, &ready_item("wi-next", "alpha")))
            .expect("seed ready item");

        assert!(
            matches!(watched_pick(&store, "alpha", None), Pick::Found(..)),
            "the seeded ready item should be picked"
        );
        cleanup(&path);
    }

    #[test]
    fn empty_pick_only_points_at_projects_that_can_actually_be_picked() {
        let path = temp_dir().join(format!("ai-board-next-{}.sqlite3", Uuid::new_v4()));
        let store = Arc::new(Store::open(&path).expect("open store"));
        store
            .with_connection(|conn| {
                insert_item(conn, &ready_item("wi-open", "beta"))?;
                let mut blocked = ready_item("wi-blocked", "gamma");
                blocked.depends_on = vec!["wi-open".to_owned()];
                insert_item(conn, &blocked)
            })
            .expect("seed items");

        let Pick::Empty { elsewhere } = watched_pick(&store, "alpha", None) else {
            panic!("project alpha holds nothing");
        };

        // gamma's only ready item waits on an unfinished dependency, so `next -p gamma`
        // would report nothing — it must not be advertised as a place to look.
        assert_eq!(elsewhere, vec!["beta".to_owned()]);
        cleanup(&path);
    }

    /// Every `pick` in a test goes through this. The bug it guards against hangs rather
    /// than fails, and a direct call would wedge the whole suite instead of failing one
    /// test — the detached thread is what turns a reintroduced deadlock into a red test.
    fn watched_pick(store: &Arc<Store>, project: &str, label: Option<&str>) -> Pick {
        let worker = Arc::clone(store);
        let project = project.to_owned();
        let label = label.map(str::to_owned);
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(pick(&worker, &project, label.as_deref()));
        });
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("next must not block on the connection guard it already holds")
            .expect("pick")
    }

    fn ready_item(id: &str, project: &str) -> WorkItem {
        let now = Local::now();
        WorkItem {
            id: id.to_owned(),
            project: project.to_owned(),
            repo_path: "/tmp".to_owned(),
            title: format!("{id} title"),
            description: String::new(),
            status: Status::Ready,
            priority: Priority::High,
            position: 0.0,
            labels: Vec::new(),
            parent_id: None,
            depends_on: Vec::new(),
            aid_task_ids: Vec::new(),
            aid_agent: None,
            aid_verify: None,
            estimate: None,
            assignee: None,
            created_by: "test".to_owned(),
            requires_approval: false,
            auto_dispatch: false,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            due_date: None,
        }
    }

    fn cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("sqlite3-shm"));
        let _ = std::fs::remove_file(path.with_extension("sqlite3-wal"));
    }
}
