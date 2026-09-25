-- Per-profile launch customization (extra_args), crash-recovery policy
-- (restart_on_crash) and per-profile stop grace period (stop_timeout_secs,
-- NULL = default 3 s). Plus the trash table that makes profile deletion
-- reversible: the profile + credential snapshot lives in `payload` (JSON) and
-- its data directory is moved to <data>/trash/<id> (recorded in trash_dir).
ALTER TABLE profiles ADD COLUMN extra_args TEXT;
ALTER TABLE profiles ADD COLUMN restart_on_crash INTEGER NOT NULL DEFAULT 0;
ALTER TABLE profiles ADD COLUMN stop_timeout_secs INTEGER;

CREATE TABLE deleted_profiles (
    id TEXT PRIMARY KEY,
    profile_name TEXT NOT NULL,
    payload TEXT NOT NULL,
    trash_dir TEXT NOT NULL,
    deleted_at INTEGER NOT NULL
);
