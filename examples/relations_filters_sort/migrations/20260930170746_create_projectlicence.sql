CREATE TABLE IF NOT EXISTS projectlicence (
    id BIGSERIAL PRIMARY KEY,
    project_id INT8 NOT NULL REFERENCES project(id),
    licence_id INT8 NOT NULL REFERENCES licence(id),
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);
