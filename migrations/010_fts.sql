-- Full-text search over recipe name, description, and ingredient text.
-- Index contents are rebuilt and maintained in Rust (see RecipeService).
CREATE VIRTUAL TABLE IF NOT EXISTS recipe_fts USING fts5(
    recipe_id UNINDEXED,
    name,
    description,
    ingredients,
    tokenize = 'porter unicode61'
);
