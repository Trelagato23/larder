-- Recipe yield output and waste (L4.2)
ALTER TABLE recipes ADD COLUMN yield_quantity TEXT;
ALTER TABLE recipes ADD COLUMN yield_unit TEXT;
ALTER TABLE recipes ADD COLUMN waste_percent TEXT;
ALTER TABLE recipe_ingredients ADD COLUMN prep_yield_percent TEXT;
