-- Ingredient unit cost (per quantity unit) or flat line cost at recipe yield
ALTER TABLE recipe_ingredients ADD COLUMN cost_per_unit TEXT;
ALTER TABLE recipe_ingredients ADD COLUMN line_cost TEXT;
-- Optional menu / sell price for food-cost %
ALTER TABLE recipes ADD COLUMN menu_price TEXT;
