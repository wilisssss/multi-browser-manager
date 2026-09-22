-- Per-profile social media / wallet credentials.
-- Secrets (password, seed phrase, private key) are NEVER stored here —
-- they live in the OS keychain keyed by `mbm-cred-<id>-<field>`.
CREATE TABLE IF NOT EXISTS credentials (
    id TEXT PRIMARY KEY,
    profile_id TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
    platform TEXT NOT NULL,
    label TEXT NOT NULL DEFAULT '',
    username TEXT,
    evm_address TEXT,
    notes TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_credentials_profile ON credentials(profile_id);
