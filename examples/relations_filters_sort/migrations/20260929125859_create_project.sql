CREATE TABLE IF NOT EXISTS project (
    id BIGSERIAL PRIMARY KEY,
    optional_string_vec TEXT[],
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);
