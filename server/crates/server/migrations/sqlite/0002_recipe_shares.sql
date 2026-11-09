-- Public share links: one unguessable token per shared recipe. Deleting the recipe
-- (or its household) removes the link.
CREATE TABLE recipe_shares (
    recipe_id TEXT PRIMARY KEY REFERENCES recipes(id) ON DELETE CASCADE,
    household_id TEXT NOT NULL REFERENCES households(id) ON DELETE CASCADE,
    token TEXT NOT NULL UNIQUE,
    created_by TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX recipe_shares_household ON recipe_shares (household_id);
