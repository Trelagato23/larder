-- Role for store-style access: manager (edit) vs kitchen (read + scale/cook)
ALTER TABLE users ADD COLUMN role TEXT NOT NULL DEFAULT 'manager';
