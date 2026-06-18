-- Tenant identity: directories, applications, users, organizations, and grants.
CREATE SCHEMA tenant;

CREATE TYPE tenant.identity_policy AS ENUM ('application_silo', 'shared_directory');
CREATE TYPE tenant.org_type AS ENUM ('personal', 'team');

CREATE TABLE tenant.directory (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    identity_policy tenant.identity_policy NOT NULL DEFAULT 'application_silo',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Default directory for single-company and startup installs (hidden in simple setups).
INSERT INTO tenant.directory (id, name, slug, identity_policy)
VALUES ('dir_00000000000000000000000001', 'Default', 'default', 'application_silo');

CREATE TABLE tenant.application (
    id TEXT PRIMARY KEY,
    directory_id TEXT NOT NULL REFERENCES tenant.directory(id) ON DELETE RESTRICT,
    client_secret_hash TEXT NOT NULL,
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_application_directory_id ON tenant.application(directory_id);

CREATE TABLE tenant."user" (
    id TEXT PRIMARY KEY,
    directory_id TEXT NOT NULL REFERENCES tenant.directory(id) ON DELETE CASCADE,
  -- Set for application_silo users; NULL for shared_directory identities.
    scoped_application_id TEXT REFERENCES tenant.application(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    email TEXT NOT NULL,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    image TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_user_directory_id ON tenant."user"(directory_id);
CREATE INDEX idx_user_scoped_application_id ON tenant."user"(scoped_application_id);

CREATE UNIQUE INDEX idx_user_shared_email
    ON tenant."user"(directory_id, email)
    WHERE scoped_application_id IS NULL;

CREATE UNIQUE INDEX idx_user_siloed_email
    ON tenant."user"(directory_id, scoped_application_id, email)
    WHERE scoped_application_id IS NOT NULL;

CREATE TABLE tenant.user_app_grant (
    user_id TEXT NOT NULL REFERENCES tenant."user"(id) ON DELETE CASCADE,
    application_id TEXT NOT NULL REFERENCES tenant.application(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, application_id)
);

CREATE INDEX idx_user_app_grant_application_id ON tenant.user_app_grant(application_id);

CREATE TABLE tenant.organization (
    id TEXT PRIMARY KEY,
    directory_id TEXT NOT NULL REFERENCES tenant.directory(id) ON DELETE CASCADE,
  -- NULL = directory-wide org (shared_directory); set = app-scoped org (application_silo).
    application_id TEXT REFERENCES tenant.application(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    org_type tenant.org_type NOT NULL,
    logo TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_organization_directory_id ON tenant.organization(directory_id);
CREATE INDEX idx_organization_application_id ON tenant.organization(application_id);

CREATE UNIQUE INDEX idx_organization_directory_slug_shared
    ON tenant.organization(directory_id, slug)
    WHERE application_id IS NULL;

CREATE UNIQUE INDEX idx_organization_directory_app_slug
    ON tenant.organization(directory_id, application_id, slug)
    WHERE application_id IS NOT NULL;

CREATE TABLE tenant.member (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL REFERENCES tenant.organization(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES tenant."user"(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'member',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT member_org_user_unique UNIQUE (organization_id, user_id)
);

CREATE INDEX idx_member_user_id ON tenant.member(user_id);

CREATE TABLE tenant.account (
    id TEXT PRIMARY KEY,
    provider_id TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES tenant."user"(id) ON DELETE CASCADE,
    password_hash TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT account_provider_user_unique UNIQUE (provider_id, user_id)
);

CREATE TABLE tenant.refresh_session (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES tenant."user"(id) ON DELETE CASCADE,
    application_id TEXT NOT NULL REFERENCES tenant.application(id) ON DELETE CASCADE,
    jti TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_refresh_session_user_id ON tenant.refresh_session(user_id);

CREATE TABLE tenant.app_invite (
    id TEXT PRIMARY KEY,
    token TEXT NOT NULL UNIQUE,
    application_id TEXT NOT NULL REFERENCES tenant.application(id) ON DELETE CASCADE,
    organization_id TEXT NOT NULL REFERENCES tenant.organization(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'member',
    name TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    accepted_at TIMESTAMPTZ,
    accepted_user_id TEXT REFERENCES tenant."user"(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_app_invite_application_id ON tenant.app_invite(application_id);
CREATE INDEX idx_app_invite_organization_id ON tenant.app_invite(organization_id);

CREATE UNIQUE INDEX idx_app_invite_pending_org_email
    ON tenant.app_invite(organization_id, email)
    WHERE accepted_at IS NULL;

CREATE TABLE admin.admin_app_grant (
    admin_user_id TEXT NOT NULL REFERENCES admin.admin_user(id) ON DELETE CASCADE,
    app_id TEXT NOT NULL REFERENCES tenant.application(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (admin_user_id, app_id)
);

CREATE INDEX idx_admin_app_grant_app_id ON admin.admin_app_grant(app_id);

CREATE TABLE admin.admin_directory_grant (
    admin_user_id TEXT NOT NULL REFERENCES admin.admin_user(id) ON DELETE CASCADE,
    directory_id TEXT NOT NULL REFERENCES tenant.directory(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (admin_user_id, directory_id)
);

CREATE INDEX idx_admin_directory_grant_directory_id ON admin.admin_directory_grant(directory_id);
