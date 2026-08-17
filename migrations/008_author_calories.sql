-- Recipe author and estimated calories (kcal per serving)
ALTER TABLE recipes ADD COLUMN author TEXT;
ALTER TABLE recipes ADD COLUMN estimated_calories INTEGER;
