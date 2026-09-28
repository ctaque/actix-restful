CREATE TABLE IF NOT EXISTS model3 (
    id BIGSERIAL PRIMARY KEY,
    field_opt TEXT NOT NULL,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);
