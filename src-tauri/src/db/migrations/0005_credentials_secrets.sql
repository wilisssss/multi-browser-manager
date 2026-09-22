-- Secrets (password, seed phrase) moved from the OS keychain into the
-- database. Private/local-only usage: the data dir is 0700 and secrets are
-- included in exports for easy migration between devices.
ALTER TABLE credentials ADD COLUMN password TEXT;
ALTER TABLE credentials ADD COLUMN seed_phrase TEXT;
