CREATE TABLE "user" (
    id TEXT PRIMARY KEY,
    app_id TEXT NOT NULL REFERENCES application(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    email TEXT NOT NULL,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    image TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT user_app_email_unique UNIQUE (app_id, email)
);

CREATE INDEX idx_user_app_id ON "user"(app_id);
