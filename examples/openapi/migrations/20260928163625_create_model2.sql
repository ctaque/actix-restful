CREATE TABLE IF NOT EXISTS model2 (
    id BIGSERIAL PRIMARY KEY,
    field1 TEXT NOT NULL,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);
