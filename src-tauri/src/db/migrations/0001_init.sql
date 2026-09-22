CREATE TABLE IF NOT EXISTS proxies (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    protocol TEXT NOT NULL,           -- http | socks5
    host TEXT NOT NULL,
    port INTEGER NOT NULL,
    username TEXT,                    -- password disimpan di OS keychain, bukan di sini
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS groups (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    color TEXT
);

CREATE TABLE IF NOT EXISTS profiles (
    id TEXT PRIMARY KEY,              -- UUID v4
    name TEXT NOT NULL UNIQUE,
    browser_type TEXT NOT NULL,       -- chrome | chromium | brave | edge
    user_data_dir TEXT NOT NULL UNIQUE,
    proxy_id TEXT REFERENCES proxies(id) ON DELETE SET NULL,
    notes TEXT,
    status TEXT NOT NULL DEFAULT 'stopped', -- stopped | running
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_used_at INTEGER
);

CREATE TABLE IF NOT EXISTS profile_groups (
    profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    group_id TEXT NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    PRIMARY KEY (profile_id, group_id)
);

CREATE TABLE IF NOT EXISTS launch_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id TEXT REFERENCES profiles(id) ON DELETE SET NULL,
    launched_at INTEGER NOT NULL,
    closed_at INTEGER,
    pid INTEGER
);

CREATE INDEX IF NOT EXISTS idx_profiles_name ON profiles(name);
CREATE INDEX IF NOT EXISTS idx_launch_history_profile ON launch_history(profile_id);
