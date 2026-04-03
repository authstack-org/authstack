CREATE TYPE org_type AS ENUM ('personal', 'team');

CREATE TABLE organization (
    id TEXT PRIMARY KEY,
    app_id TEXT NOT NULL REFERENCES application(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    org_type org_type NOT NULL DEFAULT 'team',
    logo TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT org_app_slug_unique UNIQUE (app_id, slug)
);

CREATE INDEX idx_org_app_id ON organization(app_id);
