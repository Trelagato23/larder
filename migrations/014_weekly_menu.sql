-- Store weekly menu: locked set of recipes for the floor (cap enforced in app, ~20).
CREATE TABLE IF NOT EXISTS weekly_menus (
    id TEXT PRIMARY KEY,
    week_start TEXT NOT NULL UNIQUE,
    title TEXT,
    locked INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS weekly_menu_items (
    id TEXT PRIMARY KEY,
    menu_id TEXT NOT NULL REFERENCES weekly_menus(id) ON DELETE CASCADE,
    recipe_id TEXT NOT NULL REFERENCES recipes(id) ON DELETE CASCADE,
    position INTEGER NOT NULL DEFAULT 0,
    note TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (menu_id, recipe_id)
);

CREATE INDEX IF NOT EXISTS idx_weekly_menu_items_menu ON weekly_menu_items(menu_id);
CREATE INDEX IF NOT EXISTS idx_weekly_menu_items_recipe ON weekly_menu_items(recipe_id);
