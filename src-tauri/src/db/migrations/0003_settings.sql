-- Application preferences (stored as a JSON blob under key 'app').
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
