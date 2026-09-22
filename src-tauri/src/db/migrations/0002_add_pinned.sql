-- Phase 5+: pinning support. (groups / profile_groups already exist from 0001)
ALTER TABLE profiles ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
