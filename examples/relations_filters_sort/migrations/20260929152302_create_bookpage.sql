CREATE TABLE IF NOT EXISTS bookpage (
    id BIGSERIAL PRIMARY KEY,
    book_id INT8 NOT NULL,
    page_content TEXT NOT NULL,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);
