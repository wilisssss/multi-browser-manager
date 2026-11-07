-- History of successful proxy exit IPs. Rotating proxies can return an IP
-- that was already used before, so every successful connection test records
-- the exit IP it saw; a later test warns when the exit IP is not new instead
-- of silently reusing an old one. Only successes are recorded (a failed test
-- saw no exit IP).
CREATE TABLE proxy_test_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    proxy_id TEXT NOT NULL REFERENCES proxies(id) ON DELETE CASCADE,
    ip TEXT NOT NULL,
    country TEXT,
    country_code TEXT,
    asn TEXT,
    asn_org TEXT,
    tested_at INTEGER NOT NULL
);

-- The freshness check is "has this IP ever been seen?", so it is the lookup.
CREATE INDEX idx_proxy_test_history_ip ON proxy_test_history(ip);
CREATE INDEX idx_proxy_test_history_proxy ON proxy_test_history(proxy_id);
