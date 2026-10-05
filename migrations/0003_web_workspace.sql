-- Public conversations attach to the sole current skill, not an archive version.
CREATE TABLE skill_comments (
    id TEXT PRIMARY KEY NOT NULL,
    skill_owner TEXT NOT NULL,
    skill_name TEXT NOT NULL,
    author_id TEXT NOT NULL REFERENCES accounts(owner_id),
    body TEXT NOT NULL CHECK(length(body) BETWEEN 1 AND 4096),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(skill_owner,skill_name) REFERENCES skills(owner_id,name) ON DELETE CASCADE
);
CREATE INDEX comments_skill_time ON skill_comments(skill_owner,skill_name,created_at,id);
CREATE INDEX comments_author ON skill_comments(author_id);

-- Only hashed browser secrets persist; authorization attempts expire in minutes.
CREATE TABLE web_login_transactions (
    state_hash TEXT PRIMARY KEY NOT NULL,
    browser_hash TEXT NOT NULL,
    nonce TEXT NOT NULL,
    verifier TEXT NOT NULL,
    return_to TEXT NOT NULL,
    expires_at INTEGER NOT NULL
);
CREATE INDEX web_login_expiry ON web_login_transactions(expires_at);

-- OAuth credentials remain exclusively in server storage. Refresh leases are
-- durable across isolates and ambiguous rotation is fail-closed, never replayed.
CREATE TABLE web_sessions (
    session_hash TEXT PRIMARY KEY NOT NULL,
    issuer TEXT NOT NULL,
    subject TEXT NOT NULL,
    display_name TEXT,
    csrf TEXT NOT NULL,
    access_token TEXT NOT NULL,
    id_token TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    access_expires_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    refresh_lock TEXT,
    refresh_lock_until INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX web_session_expiry ON web_sessions(expires_at);

-- Optional Identity SSO logout uses a single-use browser-bound handoff; the ID
-- token is encrypted under the session key and never returned through JSON.
CREATE TABLE web_logout_transactions (
    ticket_hash TEXT PRIMARY KEY NOT NULL,
    browser_hash TEXT NOT NULL,
    id_token TEXT NOT NULL,
    state TEXT NOT NULL,
    expires_at INTEGER NOT NULL
);
CREATE INDEX web_logout_expiry ON web_logout_transactions(expires_at);
