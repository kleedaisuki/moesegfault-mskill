-- Idempotent follow-up for environments provisioned before upload leases landed.
CREATE TABLE IF NOT EXISTS uploads (
    storage_key TEXT PRIMARY KEY NOT NULL,
    expires_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS uploads_due ON uploads(expires_at);

-- Keep one bounded scan cursor instead of restarting a large bucket's sweep.
CREATE TABLE IF NOT EXISTS maintenance (
    name TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);
