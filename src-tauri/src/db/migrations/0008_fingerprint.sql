-- Per-profile browser fingerprint settings (language, user agent, timezone,
-- geolocation, platform, system fonts, ...). Stored as one JSON document so
-- new fields don't need a schema change; NULL = all defaults (no spoofing).
ALTER TABLE profiles ADD COLUMN fingerprint TEXT;
