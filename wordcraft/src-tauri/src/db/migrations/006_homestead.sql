-- 家园建造（spec §4.2 F9，plan: docs/plans/homestead-v1.1.md）

-- 方块库存。按类型聚合，不逐块建行——方块之间没有个体差异，
-- 3657 块各占一行只是浪费
CREATE TABLE block_inventory (
  block_type TEXT PRIMARY KEY,
  owned      INTEGER NOT NULL DEFAULT 0,
  placed     INTEGER NOT NULL DEFAULT 0,

  CHECK (block_type IN ('normal', 'rare', 'limited')),
  CHECK (owned >= 0),
  -- 已放置数不可能超过拥有数。放置与移除逻辑写反时，
  -- 这条约束是最后一道防线
  CHECK (placed >= 0 AND placed <= owned)
);

-- 家园网格。只存已放置的格子，空格不占行——400 格里通常大半是空的
CREATE TABLE homestead_grid (
  x          INTEGER NOT NULL,
  y          INTEGER NOT NULL,
  block_type TEXT NOT NULL REFERENCES block_inventory(block_type),
  placed_at  TEXT NOT NULL,

  PRIMARY KEY (x, y),
  CHECK (x BETWEEN 0 AND 19),
  CHECK (y BETWEEN 0 AND 19)
);

-- 发放账本。
--
-- UNIQUE (source, source_key) 是幂等的**全部**保障：发放会在每次启动、
-- 每次会话结束后触发，没有这个约束，重启三次就发三倍方块。
-- 账本同时提供可追溯性——每一块从哪来都查得到。
CREATE TABLE block_grants (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  source     TEXT    NOT NULL,
  source_key TEXT    NOT NULL,
  block_type TEXT    NOT NULL,
  amount     INTEGER NOT NULL,
  granted_at TEXT    NOT NULL,

  UNIQUE (source, source_key),
  CHECK (source IN ('mastery', 'streak', 'milestone')),
  CHECK (amount > 0)
);

CREATE INDEX idx_grants_source ON block_grants(source);

-- 三种类型预建行，避免发放时还要判断行是否存在
INSERT OR IGNORE INTO block_inventory (block_type) VALUES
  ('normal'), ('rare'), ('limited');
