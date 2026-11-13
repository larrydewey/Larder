-- Trial recipes: recipes the household wants to try before they join the cookbook.
-- Existing recipes stay in the cookbook. tried_count / last_tried_at record how
-- often a recipe was made ("I made this"); they are household-wide, not per user.
ALTER TABLE recipes ADD COLUMN status TEXT NOT NULL DEFAULT 'cookbook';
ALTER TABLE recipes ADD COLUMN tried_count BIGINT NOT NULL DEFAULT 0;
ALTER TABLE recipes ADD COLUMN last_tried_at TEXT;
CREATE INDEX idx_recipes_household_status ON recipes(household_id, status);

-- The "Did you make it?" question on past meal plan entries is answered once:
-- trial_outcome is keep | again | discard | skipped, trial_resolved_at is when.
ALTER TABLE meal_plan ADD COLUMN trial_outcome TEXT;
ALTER TABLE meal_plan ADD COLUMN trial_resolved_at TEXT;
