CREATE TABLE projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    content TEXT NOT NULL,
    deleted_at TEXT,
    updated_at TEXT,
    created_at TEXT
);

CREATE TABLE items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL REFERENCES projects (id),
    content TEXT NOT NULL,
    deleted_at TEXT,
    updated_at TEXT,
    created_at TEXT
);

CREATE INDEX items_project_id ON items (project_id);
