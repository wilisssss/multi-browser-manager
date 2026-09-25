//! Startup maintenance tasks, extracted from `run()` so they are unit-testable
//! against an in-memory database instead of living inline in the setup hook.

use crate::browser::launcher;
use crate::commands::settings::Settings;
use crate::error::AppResult;
use rusqlite::Connection;
use std::path::Path;

/// Reconciles DB profile statuses with what is actually running and returns
/// the ids whose status was corrected.
///
/// A browser may have outlived the previous MBM session (orphan with a live
/// SingletonLock). If its pid is alive AND its cmdline references this
/// profile's data dir, keep it as 'running' (stop_profile can still kill it
/// via the lock); any stale 'running' row without a live browser converges
/// to 'stopped' and its leftover lock file is removed so the next launch
/// works.
///
/// Note: orphan detection relies on the Chromium `SingletonLock` symlink and
/// `/proc` inspection, which are Linux-only (see `launcher::singleton_pid`).
/// On other platforms a browser from a previous session always converges to
/// 'stopped' here; profiles this session actually spawned stay accurate via
/// the watcher tasks.
pub fn reconcile_statuses(conn: &Connection) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare("SELECT id, user_data_dir, status FROM profiles")?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut corrected = Vec::new();
    for (id, dir, status) in rows {
        let alive = launcher::singleton_pid(std::path::Path::new(&dir))
            .filter(|pid| launcher::pid_alive(*pid) && launcher::pid_cmdline_contains(*pid, &dir))
            .is_some();

        let desired = if alive { "running" } else { "stopped" };
        if status != desired {
            conn.execute(
                "UPDATE profiles SET status = ?1 WHERE id = ?2",
                rusqlite::params![desired, id],
            )?;
            // Clean the lock of a dead browser so the next launch works.
            if !alive {
                let _ = std::fs::remove_file(std::path::Path::new(&dir).join("SingletonLock"));
            }
            corrected.push(id);
        }
    }
    Ok(corrected)
}

/// Prunes launch history per user settings: drops closed entries older than
/// the retention window and caps the table size (newest are kept).
/// The cap only counts CLOSED entries — a long-running session's open row is
/// never dropped, otherwise it could not be closed (duration recorded) when
/// the browser eventually exits.
/// Non-fatal: a failed prune only means the history grows past its budget,
/// so errors are logged, not propagated.
pub fn prune_launch_history(conn: &Connection, settings: &Settings) {
    let retention = settings.history_retention_days.clamp(1, 3650);
    let max_entries = settings.history_max_entries.clamp(10, 100_000);
    let cutoff = chrono::Utc::now().timestamp() - retention * 24 * 3600;
    if let Err(e) = conn.execute(
        "DELETE FROM launch_history WHERE closed_at IS NOT NULL AND closed_at < ?1",
        rusqlite::params![cutoff],
    ) {
        eprintln!("mbm: failed to prune launch history by age: {e}");
    }
    if let Err(e) = conn.execute(
        &format!(
            "DELETE FROM launch_history
             WHERE closed_at IS NOT NULL
               AND id NOT IN (
                   SELECT id FROM launch_history
                   WHERE closed_at IS NOT NULL
                   ORDER BY launched_at DESC LIMIT {max_entries}
               )"
        ),
        [],
    ) {
        eprintln!("mbm: failed to cap launch history size: {e}");
    }
}

/// Purges trash snapshots (feature 6) older than the retention window.
/// Non-fatal at startup: a failed purge only means trashed data lingers a
/// bit longer, so errors are logged, not propagated.
pub fn purge_expired_trash(conn: &Connection, profiles_root: &Path) -> AppResult<usize> {
    crate::services::trash::purge_expired_trash(conn, profiles_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use rusqlite::params;
    use std::path::{Path, PathBuf};

    fn mem_conn() -> Connection {
        db::init(Path::new(":memory:")).expect("in-memory db")
    }

    fn insert_profile(conn: &Connection, id: &str, dir: &str, status: &str) {
        conn.execute(
            "INSERT INTO profiles (id, name, browser_type, user_data_dir, status, created_at, updated_at)
             VALUES (?1, ?1, 'chromium', ?2, ?3, 1, 1)",
            params![id, dir, status],
        )
        .unwrap();
    }

    fn tmp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mbm-startup-test-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn stale_running_rows_converge_to_stopped() {
        let conn = mem_conn();
        // No SingletonLock exists → no live browser → must converge to stopped.
        let dir = tmp_dir("stale");
        insert_profile(&conn, "p1", dir.to_str().unwrap(), "running");

        let corrected = reconcile_statuses(&conn).unwrap();
        assert_eq!(corrected, vec!["p1".to_string()]);

        let status: String = conn
            .query_row("SELECT status FROM profiles WHERE id = 'p1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "stopped");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn stopped_rows_are_left_untouched() {
        let conn = mem_conn();
        let dir = tmp_dir("untouched");
        insert_profile(&conn, "p1", dir.to_str().unwrap(), "stopped");

        let corrected = reconcile_statuses(&conn).unwrap();
        // Nothing was wrong — no writes, ids not reported as corrected.
        assert!(corrected.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn lock_of_a_dead_browser_is_cleaned() {
        let conn = mem_conn();
        let dir = tmp_dir("lock");
        insert_profile(&conn, "p1", dir.to_str().unwrap(), "running");

        // Symlink target format "<hostname>-<pid>"; pid 0 is never alive.
        #[cfg(unix)]
        std::os::unix::fs::symlink("somehost-0", dir.join("SingletonLock")).unwrap();

        let corrected = reconcile_statuses(&conn).unwrap();
        assert_eq!(corrected, vec!["p1".to_string()]);
        assert!(!dir.join("SingletonLock").exists(), "stale lock must be removed");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn prunes_old_closed_and_excess_entries() {
        let conn = mem_conn();
        insert_profile(&conn, "p1", "/tmp/mbm/p1", "stopped");
        let now = chrono::Utc::now().timestamp();
        let day = 24 * 3600;

        // 200-day-old closed entry: beyond the 90-day default retention.
        conn.execute(
            "INSERT INTO launch_history (profile_id, launched_at, closed_at) VALUES ('p1', ?1, ?1)",
            params![now - 200 * day],
        )
        .unwrap();
        // Open entry with an old launch: must survive (still running).
        conn.execute(
            "INSERT INTO launch_history (profile_id, launched_at) VALUES ('p1', ?1)",
            params![now - 500 * day],
        )
        .unwrap();
        // 15 recent closed entries: with a cap of 10, the 5 oldest go.
        for i in 0..15 {
            conn.execute(
                "INSERT INTO launch_history (profile_id, launched_at, closed_at) VALUES ('p1', ?1, ?1)",
                params![now - i],
            )
            .unwrap();
        }

        let settings = Settings {
            history_max_entries: 10,
            ..Settings::default()
        };
        prune_launch_history(&conn, &settings);

        let (total, has_open): (i64, i64) = conn.query_row(
            "SELECT COUNT(*), SUM(CASE WHEN closed_at IS NULL THEN 1 ELSE 0 END) FROM launch_history",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert_eq!(has_open, 1, "open entries are never pruned");
        assert_eq!(total, 11, "cap keeps exactly 10 newest closed + the open row");
        let oldest: i64 = conn
            .query_row(
                "SELECT MIN(closed_at) FROM launch_history WHERE closed_at IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(oldest > now - 10, "200-day-old entry must be gone");
    }
}
