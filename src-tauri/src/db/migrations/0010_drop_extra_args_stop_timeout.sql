-- Removes the per-profile extra launch arguments and the per-profile stop
-- grace period. Both were unused by the workflow; the stop grace period now
-- uses the fixed default in the process supervisor.
ALTER TABLE profiles DROP COLUMN extra_args;
ALTER TABLE profiles DROP COLUMN stop_timeout_secs;
