-- Metadata cleanup: typo units, merge alias tags, drop dead hybrids.

-- ChefTec typo: "8 0z" / "0 0z" → oz
UPDATE recipes
SET description = REPLACE(description, '0z', 'oz')
WHERE description LIKE '%0z%';

-- dressing → dressings (duplicate chip)
INSERT OR IGNORE INTO recipe_tags (recipe_id, tag_id)
SELECT rt.recipe_id, d.id
FROM recipe_tags rt
JOIN tags t ON t.id = rt.tag_id
JOIN tags d ON lower(d.name) = 'dressings'
WHERE lower(t.name) = 'dressing';

DELETE FROM recipe_tags
WHERE tag_id IN (SELECT id FROM tags WHERE lower(name) = 'dressing');

-- Compound grab-go leftovers → real depts
INSERT OR IGNORE INTO recipe_tags (recipe_id, tag_id)
SELECT rt.recipe_id, s.id
FROM recipe_tags rt
JOIN tags t ON t.id = rt.tag_id
JOIN tags s ON lower(s.name) = 'sandwiches'
WHERE lower(t.name) = 'grab-go-sandwiches';

INSERT OR IGNORE INTO recipe_tags (recipe_id, tag_id)
SELECT rt.recipe_id, s.id
FROM recipe_tags rt
JOIN tags t ON t.id = rt.tag_id
JOIN tags s ON lower(s.name) = 'bakery'
WHERE lower(t.name) = 'bakery-grab-go';

INSERT OR IGNORE INTO recipe_tags (recipe_id, tag_id)
SELECT rt.recipe_id, s.id
FROM recipe_tags rt
JOIN tags t ON t.id = rt.tag_id
JOIN tags s ON lower(s.name) = 'pizza'
WHERE lower(t.name) = 'grab-go-pizza';

INSERT OR IGNORE INTO recipe_tags (recipe_id, tag_id)
SELECT rt.recipe_id, s.id
FROM recipe_tags rt
JOIN tags t ON t.id = rt.tag_id
JOIN tags s ON lower(s.name) = 'grab-go'
WHERE lower(t.name) = 'cheese-grab-go';

DELETE FROM recipe_tags
WHERE tag_id IN (
    SELECT id FROM tags WHERE lower(name) IN (
        'grab-go-sandwiches', 'bakery-grab-go', 'grab-go-pizza',
        'cheese-grab-go', 'grab-go-hot-bar', 'dressing'
    )
);
