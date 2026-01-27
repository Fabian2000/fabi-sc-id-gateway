-- Add admin_origin to id_config and allowed_users to routes

ALTER TABLE id_config ADD COLUMN admin_origin TEXT;

-- Route allowed users (stored as JSON array)
ALTER TABLE routes ADD COLUMN allowed_users TEXT NOT NULL DEFAULT '[]';
