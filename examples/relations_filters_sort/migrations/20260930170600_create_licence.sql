CREATE TABLE IF NOT EXISTS licence (
    id BIGSERIAL PRIMARY KEY,
    licence_name TEXT NOT NULL,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);
