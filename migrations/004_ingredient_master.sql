-- Shared ingredient master: one price, many recipes
CREATE TABLE IF NOT EXISTS ingredients (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    name_key TEXT NOT NULL UNIQUE,
    default_unit TEXT,
    cost_per_unit TEXT,
    pack_size TEXT,
    pack_unit TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_ingredients_name_key ON ingredients(name_key);

-- Link recipe lines to master (NULL = free-text / unlinked)
ALTER TABLE recipe_ingredients ADD COLUMN master_ingredient_id TEXT REFERENCES ingredients(id);
