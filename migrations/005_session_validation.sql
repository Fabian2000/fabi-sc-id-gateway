-- Add last_validated timestamp to sessions for periodic token validation
-- SQLite doesn't allow non-constant defaults in ALTER TABLE, so we use a constant
-- Existing sessions will be re-validated on next access
ALTER TABLE sessions ADD COLUMN last_validated TEXT NOT NULL DEFAULT '1970-01-01T00:00:00Z';
