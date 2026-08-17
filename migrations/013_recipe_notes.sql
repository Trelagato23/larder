-- Floor notes: subtle tips or flagged alerts, with optional manager signature.
CREATE TABLE IF NOT EXISTS recipe_notes (
    id TEXT PRIMARY KEY,
    recipe_id TEXT NOT NULL REFERENCES recipes(id) ON DELETE CASCADE,
    body TEXT NOT NULL,
    severity TEXT NOT NULL DEFAULT 'subtle'
        CHECK (severity IN ('subtle', 'flagged')),
    author_role TEXT NOT NULL DEFAULT 'team'
        CHECK (author_role IN ('team', 'supervisor', 'manager')),
    author_name TEXT NOT NULL,
    author_user_id TEXT,
    signature TEXT,
    signed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_recipe_notes_recipe ON recipe_notes(recipe_id);
CREATE INDEX IF NOT EXISTS idx_recipe_notes_severity ON recipe_notes(recipe_id, severity);
