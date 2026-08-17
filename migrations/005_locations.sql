-- Multi-store: shared recipe library, per-location ingredient pricing
CREATE TABLE IF NOT EXISTS locations (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS user_locations (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    location_id TEXT NOT NULL REFERENCES locations(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, location_id)
);

CREATE TABLE IF NOT EXISTS location_ingredient_prices (
    location_id TEXT NOT NULL REFERENCES locations(id) ON DELETE CASCADE,
    ingredient_id TEXT NOT NULL REFERENCES ingredients(id) ON DELETE CASCADE,
    cost_per_unit TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (location_id, ingredient_id)
);

CREATE INDEX IF NOT EXISTS idx_location_prices_ingredient ON location_ingredient_prices(ingredient_id);
