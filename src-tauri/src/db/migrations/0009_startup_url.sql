-- Page opened when the profile's browser starts (NULL = browser default).
ALTER TABLE profiles ADD COLUMN startup_url TEXT;
