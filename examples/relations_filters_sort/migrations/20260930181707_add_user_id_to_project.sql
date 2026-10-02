-- Add migration script here

ALTER TABLE project ADD COLUMN IF NOT EXISTS user_id INT8 NOT NULL REFERENCES users (id);
