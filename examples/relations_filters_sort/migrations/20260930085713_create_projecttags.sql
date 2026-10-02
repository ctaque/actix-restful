CREATE TABLE IF NOT EXISTS projecttags (
    id BIGSERIAL PRIMARY KEY,
    project_id INT8 REFERENCES project(id),
    tag_id INT8 REFERENCES tags(id),
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);
