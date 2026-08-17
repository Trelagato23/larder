-- Service stations: Grab & Go (cold) vs Hot Bar (hot) are mutually exclusive.
-- ChefTec left many hot items tagged both grab-go and grab-go-hot-bar.

-- Ensure every former grab-go-hot-bar recipe has canonical hot-bar.
INSERT OR IGNORE INTO recipe_tags (recipe_id, tag_id)
SELECT rt.recipe_id, hb.id
FROM recipe_tags rt
JOIN tags t ON t.id = rt.tag_id
JOIN tags hb ON lower(hb.name) = 'hot-bar'
WHERE lower(t.name) = 'grab-go-hot-bar';

-- Cold grab-go / grab-and-go must not sit on hot-bar recipes.
DELETE FROM recipe_tags
WHERE tag_id IN (
    SELECT id FROM tags WHERE lower(name) IN ('grab-go', 'grab-and-go')
)
AND recipe_id IN (
    SELECT rt.recipe_id
    FROM recipe_tags rt
    JOIN tags t ON t.id = rt.tag_id
    WHERE lower(t.name) IN ('hot-bar', 'grab-go-hot-bar')
);

-- Drop the hybrid tag; hot-bar is the only hot station tag.
DELETE FROM recipe_tags
WHERE tag_id IN (
    SELECT id FROM tags WHERE lower(name) = 'grab-go-hot-bar'
);

-- Alias: grab-and-go → grab-go (cold).
INSERT OR IGNORE INTO recipe_tags (recipe_id, tag_id)
SELECT rt.recipe_id, gg.id
FROM recipe_tags rt
JOIN tags t ON t.id = rt.tag_id
JOIN tags gg ON lower(gg.name) = 'grab-go'
WHERE lower(t.name) = 'grab-and-go';

DELETE FROM recipe_tags
WHERE tag_id IN (
    SELECT id FROM tags WHERE lower(name) = 'grab-and-go'
);
