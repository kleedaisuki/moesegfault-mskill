-- Identity subjects stay private; opaque publisher IDs are the public namespace.
CREATE TABLE IF NOT EXISTS accounts (
    owner_id TEXT PRIMARY KEY NOT NULL,
    issuer TEXT NOT NULL,
    subject TEXT NOT NULL,
    display_name TEXT,
    created_at TEXT NOT NULL,
    UNIQUE (issuer, subject)
);

-- A package has exactly one current pointer. Storage keys are write-unique, not versions.
CREATE TABLE IF NOT EXISTS skills (
    owner_id TEXT NOT NULL REFERENCES accounts(owner_id),
    name TEXT NOT NULL,
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
    size_bytes INTEGER NOT NULL CHECK (size_bytes > 0 AND size_bytes <= 16777216),
    description TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    storage_key TEXT NOT NULL UNIQUE,
    PRIMARY KEY (owner_id, name)
);
CREATE INDEX IF NOT EXISTS skills_updated ON skills(updated_at DESC, owner_id, name);

-- Durable outbox: blob deletion failure never undoes a committed publication.
CREATE TABLE IF NOT EXISTS garbage (
    storage_key TEXT PRIMARY KEY NOT NULL,
    delete_after INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS garbage_due ON garbage(delete_after);
