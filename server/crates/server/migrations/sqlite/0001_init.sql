CREATE TABLE households (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL,
    username_lower TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    email TEXT,
    email_lower TEXT UNIQUE,
    password_hash TEXT,
    oidc_subject TEXT UNIQUE,
    is_admin INTEGER NOT NULL DEFAULT 0,
    disabled INTEGER NOT NULL DEFAULT 0,
    household_id TEXT NOT NULL REFERENCES households(id),
    household_role TEXT NOT NULL DEFAULT 'member',
    preferences TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_users_household ON users(household_id);

CREATE TABLE invites (
    code TEXT PRIMARY KEY,
    household_id TEXT NOT NULL REFERENCES households(id) ON DELETE CASCADE,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE TABLE refresh_tokens (
    token_hash TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);
CREATE INDEX idx_refresh_user ON refresh_tokens(user_id);

CREATE TABLE recipes (
    id TEXT PRIMARY KEY,
    household_id TEXT NOT NULL REFERENCES households(id) ON DELETE CASCADE,
    slug TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    yield_quantity REAL,
    yield_unit TEXT NOT NULL DEFAULT 'servings',
    prep_time_minutes INTEGER,
    cook_time_minutes INTEGER,
    total_time_minutes INTEGER,
    source_url TEXT,
    source_name TEXT,
    notes TEXT NOT NULL DEFAULT '',
    rating INTEGER,
    ingredients TEXT NOT NULL DEFAULT '[]',
    instructions TEXT NOT NULL DEFAULT '[]',
    tags TEXT NOT NULL DEFAULT '[]',
    categories TEXT NOT NULL DEFAULT '[]',
    nutrition TEXT NOT NULL DEFAULT '{}',
    image_id TEXT,
    search_text TEXT NOT NULL DEFAULT '',
    name_lower TEXT NOT NULL DEFAULT '',
    created_by TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    UNIQUE (household_id, slug)
);
CREATE INDEX idx_recipes_household_updated ON recipes(household_id, updated_at);
CREATE INDEX idx_recipes_household_name ON recipes(household_id, name_lower);

CREATE TABLE recipe_tags (
    recipe_id TEXT NOT NULL REFERENCES recipes(id) ON DELETE CASCADE,
    household_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    name_lower TEXT NOT NULL,
    PRIMARY KEY (recipe_id, kind, name_lower)
);
CREATE INDEX idx_recipe_tags_lookup ON recipe_tags(household_id, kind, name_lower);

CREATE TABLE favorites (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    recipe_id TEXT NOT NULL REFERENCES recipes(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    PRIMARY KEY (user_id, recipe_id)
);

CREATE TABLE deleted_recipes (
    id TEXT PRIMARY KEY,
    household_id TEXT NOT NULL,
    deleted_at TEXT NOT NULL
);
CREATE INDEX idx_deleted_recipes ON deleted_recipes(household_id, deleted_at);

CREATE TABLE meal_plan (
    id TEXT PRIMARY KEY,
    household_id TEXT NOT NULL REFERENCES households(id) ON DELETE CASCADE,
    date TEXT NOT NULL,
    meal TEXT NOT NULL,
    recipe_id TEXT REFERENCES recipes(id) ON DELETE SET NULL,
    title TEXT,
    note TEXT,
    servings REAL,
    created_by TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_meal_plan ON meal_plan(household_id, date);

CREATE TABLE shopping_lists (
    id TEXT PRIMARY KEY,
    household_id TEXT NOT NULL REFERENCES households(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_shopping_lists ON shopping_lists(household_id);

CREATE TABLE shopping_items (
    id TEXT PRIMARY KEY,
    list_id TEXT NOT NULL REFERENCES shopping_lists(id) ON DELETE CASCADE,
    food TEXT,
    quantity REAL,
    unit TEXT,
    note TEXT,
    aisle TEXT NOT NULL DEFAULT 'Other',
    checked INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL DEFAULT 0,
    recipe_ids TEXT NOT NULL DEFAULT '[]',
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_shopping_items ON shopping_items(list_id);

CREATE TABLE import_jobs (
    id TEXT PRIMARY KEY,
    household_id TEXT NOT NULL REFERENCES households(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    source TEXT NOT NULL,
    status TEXT NOT NULL,
    total INTEGER NOT NULL DEFAULT 0,
    imported INTEGER NOT NULL DEFAULT 0,
    skipped INTEGER NOT NULL DEFAULT 0,
    failed INTEGER NOT NULL DEFAULT 0,
    errors TEXT NOT NULL DEFAULT '[]',
    recipe_ids TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    finished_at TEXT
);
CREATE INDEX idx_import_jobs ON import_jobs(household_id, created_at);
