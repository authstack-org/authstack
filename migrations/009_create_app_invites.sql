CREATE TABLE app_invite (
    id TEXT PRIMARY KEY,
    token TEXT NOT NULL UNIQUE,
    app_id TEXT NOT NULL REFERENCES application(id) ON DELETE CASCADE,
    organization_id TEXT NOT NULL REFERENCES organization(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'member',
    name TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    accepted_at TIMESTAMPTZ,
    accepted_user_id TEXT REFERENCES "user"(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_app_invite_token ON app_invite(token);
CREATE INDEX idx_app_invite_org_id ON app_invite(organization_id);
CREATE INDEX idx_app_invite_app_id ON app_invite(app_id);

CREATE UNIQUE INDEX idx_app_invite_pending_org_email
    ON app_invite(organization_id, email)
    WHERE accepted_at IS NULL;
