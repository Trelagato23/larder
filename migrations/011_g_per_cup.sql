-- Density for cup↔g conversions (nullable; heuristics when null)
ALTER TABLE ingredients ADD COLUMN g_per_cup TEXT;
