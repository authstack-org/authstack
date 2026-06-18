ALTER TABLE admin_user
    ADD COLUMN role TEXT NOT NULL DEFAULT 'instance_admin'
    CHECK (role IN ('instance_admin', 'app_admin'));

CREATE TABLE admin_app_grant (
    admin_user_id TEXT NOT NULL REFERENCES admin_user(id) ON DELETE CASCADE,
    app_id        TEXT NOT NULL REFERENCES application(id) ON DELETE CASCADE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (admin_user_id, app_id)
);

CREATE INDEX idx_admin_app_grant_app_id ON admin_app_grant(app_id);
