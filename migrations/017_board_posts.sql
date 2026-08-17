-- General floor posts (not tied to a recipe). Recipe notes stay on recipe_notes.
CREATE TABLE IF NOT EXISTS board_posts (
    id TEXT PRIMARY KEY,
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
CREATE INDEX IF NOT EXISTS idx_board_posts_created ON board_posts(created_at);
CREATE INDEX IF NOT EXISTS idx_board_posts_severity ON board_posts(severity);
