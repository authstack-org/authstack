-- Platform operators (instance / app admins). Isolated from tenant identity data.
CREATE SCHEMA admin;

CREATE TABLE admin.admin_user (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'instance_admin',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT admin_user_role_check CHECK (role IN ('instance_admin', 'app_admin', 'directory_admin'))
);
