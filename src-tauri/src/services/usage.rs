//! Usage statistics (feature recommendation 5): aggregate launch_history
//! into per-profile session counts and total session time. Pure query —
//! the data is already recorded by the process watcher.

use crate::error::AppResult;
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStat {
    pub profile_id: String,
    pub sessions: i64,
    /// Total session seconds; open sessions count up to "now".
    pub seconds: i64,
}

pub fn usage_stats(conn: &Connection) -> AppResult<Vec<UsageStat>> {
    let mut stmt = conn.prepare(
        "SELECT profile_id,
                COUNT(*),
                SUM(COALESCE(closed_at, CAST(strftime('%s', 'now') AS INTEGER)) - launched_at)
         FROM launch_history
         GROUP BY profile_id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(UsageStat {
                profile_id: r.get(0)?,
                sessions: r.get(1)?,
                seconds: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use rusqlite::params;
    use std::path::Path;

    #[test]
    fn aggregates_sessions_and_seconds() {
        let conn = db::init(Path::new(":memory:")).unwrap();
        conn.execute(
            "INSERT INTO profiles (id, name, browser_type, user_data_dir, status, created_at, updated_at)
             VALUES ('p1', 'A', 'chromium', '/tmp/a', 'stopped', 1, 1)",
            [],
        )
        .unwrap();
        let now = chrono::Utc::now().timestamp();
        // Two closed sessions: 3600 s and 1800 s.
        conn.execute(
            "INSERT INTO launch_history (profile_id, launched_at, closed_at) VALUES ('p1', ?1, ?2)",
            params![now - 7200, now - 3600],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO launch_history (profile_id, launched_at, closed_at) VALUES ('p1', ?1, ?2)",
            params![now - 1800, now],
        )
        .unwrap();
        // One open session: counts as (now - launched_at).
        conn.execute(
            "INSERT INTO launch_history (profile_id, launched_at) VALUES ('p1', ?1)",
            params![now - 600],
        )
        .unwrap();

        let stats = usage_stats(&conn).unwrap();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].profile_id, "p1");
        assert_eq!(stats[0].sessions, 3);
        let open_seconds = 600; // up to rounding between queries
        assert!(
            stats[0].seconds >= 3600 + 1800 + open_seconds - 2
                && stats[0].seconds <= 3600 + 1800 + open_seconds + 2,
            "unexpected total {}",
            stats[0].seconds
        );
    }
}
