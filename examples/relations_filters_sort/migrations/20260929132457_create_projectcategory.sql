CREATE TABLE IF NOT EXISTS projectcategory (
    id BIGSERIAL PRIMARY KEY,
    category_id INT8 REFERENCES category(id),
    project_id INT8 REFERENCES project(id),
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);
