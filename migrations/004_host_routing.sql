-- Change from path_prefix routing to host-based routing

-- Create new routes table
CREATE TABLE routes_new (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    host TEXT NOT NULL UNIQUE,
    upstream_url TEXT NOT NULL,
    requires_auth INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,
    allowed_users TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Copy data (use path_prefix as host for migration)
INSERT INTO routes_new (id, name, host, upstream_url, requires_auth, enabled, allowed_users, created_at, updated_at)
SELECT id, name, path_prefix, upstream_url, requires_auth, enabled, allowed_users, created_at, updated_at FROM routes;

-- Drop old table and rename
DROP TABLE routes;
ALTER TABLE routes_new RENAME TO routes;

-- Recreate index
CREATE INDEX idx_routes_host ON routes(host);
