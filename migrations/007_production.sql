-- Daily production plan + pull list (L4.3)
CREATE TABLE IF NOT EXISTS production_plans (
    id TEXT PRIMARY KEY,
    location_id TEXT REFERENCES locations(id),
    plan_date TEXT NOT NULL,
    title TEXT,
    notes TEXT,
    user_id TEXT NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS production_plan_items (
    id TEXT PRIMARY KEY,
    plan_id TEXT NOT NULL REFERENCES production_plans(id) ON DELETE CASCADE,
    recipe_id TEXT NOT NULL REFERENCES recipes(id),
    batches TEXT NOT NULL DEFAULT '1',
    servings_override INTEGER,
    position INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_production_plans_date ON production_plans(plan_date);
CREATE INDEX IF NOT EXISTS idx_production_items_plan ON production_plan_items(plan_id);
