-- Tenant identity: directories, applications, users, organizations, and grants.
CREATE SCHEMA tenant;

CREATE TABLE tenant.directory (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Default directory for single-company installs (hidden in simple setups).
INSERT INTO tenant.directory (id, name, slug)
VALUES ('dir_00000000000000000000000001', 'Default', 'default');

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
    name TEXT NOT NULL,
    email TEXT NOT NULL,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    image TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT user_directory_email_unique UNIQUE (directory_id, email)
);

CREATE INDEX idx_user_directory_id ON tenant."user"(directory_id);

CREATE TABLE tenant.user_app_grant (
    user_id TEXT NOT NULL REFERENCES tenant."user"(id) ON DELETE CASCADE,
    application_id TEXT NOT NULL REFERENCES tenant.application(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, application_id)
);

CREATE INDEX idx_user_app_grant_application_id ON tenant.user_app_grant(application_id);

CREATE TABLE tenant.app_permission (
    id TEXT PRIMARY KEY,
    application_id TEXT NOT NULL REFERENCES tenant.application(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT app_permission_application_key_unique UNIQUE (application_id, key)
);

CREATE INDEX idx_app_permission_application_id ON tenant.app_permission(application_id);

CREATE TABLE tenant.organization (
    id TEXT PRIMARY KEY,
    directory_id TEXT NOT NULL REFERENCES tenant.directory(id) ON DELETE CASCADE,
    application_id TEXT NOT NULL REFERENCES tenant.application(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL,
    logo TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT organization_application_slug_unique UNIQUE (application_id, slug)
);

CREATE INDEX idx_organization_directory_id ON tenant.organization(directory_id);
CREATE INDEX idx_organization_application_id ON tenant.organization(application_id);

CREATE TABLE tenant.org_role (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL REFERENCES tenant.organization(id) ON DELETE CASCADE,
    slug TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT org_role_organization_slug_unique UNIQUE (organization_id, slug)
);

CREATE INDEX idx_org_role_organization_id ON tenant.org_role(organization_id);

CREATE TABLE tenant.org_role_permission (
    org_role_id TEXT NOT NULL REFERENCES tenant.org_role(id) ON DELETE CASCADE,
    app_permission_id TEXT NOT NULL REFERENCES tenant.app_permission(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (org_role_id, app_permission_id)
);

CREATE INDEX idx_org_role_permission_app_permission_id ON tenant.org_role_permission(app_permission_id);

CREATE TABLE tenant.member (
    id TEXT PRIMARY KEY,
    organization_id TEXT NOT NULL REFERENCES tenant.organization(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES tenant."user"(id) ON DELETE CASCADE,
    org_role_id TEXT NOT NULL REFERENCES tenant.org_role(id) ON DELETE RESTRICT,
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
    org_role_id TEXT NOT NULL REFERENCES tenant.org_role(id) ON DELETE RESTRICT,
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
