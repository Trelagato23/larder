-- Track kitchen use so rarely opened recipes sink in the default list.
-- last_opened_at NULL = never opened (sorts last).
ALTER TABLE recipes ADD COLUMN last_opened_at TEXT;
ALTER TABLE recipes ADD COLUMN open_count INTEGER NOT NULL DEFAULT 0;
