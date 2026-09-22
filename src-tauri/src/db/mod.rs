pub mod migrations {
    /// Ordered list of applied migrations: (version, sql).
    /// New migrations must be appended, never reordered.
    pub const MIGRATIONS: &[(i64, &str)] = &[
        (1, include_str!("migrations/0001_init.sql")),
        (2, include_str!("migrations/0002_add_pinned.sql")),
        (3, include_str!("migrations/0003_settings.sql")),
        (4, include_str!("migrations/0004_credentials.sql")),
        (5, include_str!("migrations/0005_credentials_secrets.sql")),
    ];
}

use crate::error::AppResult;
use rusqlite::Connection;
use std::path::Path;

/// Opens (or creates) the SQLite database at `db_path` and applies pending migrations.
pub fn init(db_path: &Path) -> AppResult<Connection> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    // Wait instead of failing immediately when another process (e.g. `mbm
    // --list` from a WM keybind) holds the write lock.
    conn.busy_timeout(std::time::Duration::from_secs(2))?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );",
    )?;

    let current: i64 =
        conn.query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |r| {
            r.get(0)
        })?;

    for &(version, sql) in migrations::MIGRATIONS {
        if version > current {
            // Apply each migration atomically: a crash mid-migration must not
            // leave a partial schema without a version record (which would make
            // the next startup fail permanently with e.g. "duplicate column").
            conn.execute_batch(&format!(
                "BEGIN IMMEDIATE;\n{sql}\nCOMMIT;"
            ))?;
            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                rusqlite::params![version, chrono::Utc::now().timestamp()],
            )?;
        }
    }

    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Unique per-test temp directory so parallel tests never collide.
    fn tmp_db(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mbm-test-{}-{label}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        dir.join("test.sqlite3")
    }

    #[test]
    fn migrates_fresh_database() {
        let path = tmp_db("fresh");
        let _ = std::fs::remove_file(&path);

        let conn = init(&path).expect("init should succeed");
        let applied: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(applied, migrations::MIGRATIONS.len() as i64);

        // Core tables exist.
        let tables: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN
                 ('profiles', 'proxies', 'groups', 'profile_groups', 'launch_history')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 5);

        // Migration 2's pinned column is present.
        let pinned: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('profiles') WHERE name = 'pinned'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pinned, 1);

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn re_init_is_idempotent() {
        let path = tmp_db("idem");
        let _ = std::fs::remove_file(&path);

        let first = init(&path).expect("first init");
        drop(first);
        let second = init(&path).expect("second init must not re-run migrations");

        let applied: i64 = second
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r.get(0))
            .unwrap();
        assert_eq!(applied, migrations::MIGRATIONS.len() as i64);

        // Re-running must not duplicate columns (e.g. "duplicate column: pinned").
        let pinned: i64 = second
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('profiles') WHERE name = 'pinned'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pinned, 1);

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
