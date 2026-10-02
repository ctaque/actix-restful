CREATE TABLE IF NOT EXISTS book (
    id BIGSERIAL PRIMARY KEY,
    isbn TEXT NOT NULL,
    author TEXT[] NOT NULL,
    title TEXT NOT NULL,
    project_id INT8,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);
