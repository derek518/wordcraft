-- 魔王讨伐战（spec §4.2 F10）
--
-- 击败魔王掉落稀有方块，这是 spec 为稀有方块设定的**原始来源**；
-- 里程碑（migration 006 时的临时替代）继续保留，两者并存。
--
-- SQLite 不能修改 CHECK 约束，只能重建表。

CREATE TABLE block_grants_new (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  source     TEXT    NOT NULL,
  source_key TEXT    NOT NULL,
  block_type TEXT    NOT NULL,
  amount     INTEGER NOT NULL,
  granted_at TEXT    NOT NULL,

  UNIQUE (source, source_key),
  CHECK (source IN ('mastery', 'streak', 'milestone', 'boss')),
  CHECK (amount > 0)
);

INSERT INTO block_grants_new SELECT * FROM block_grants;
DROP TABLE block_grants;
ALTER TABLE block_grants_new RENAME TO block_grants;

-- 索引随旧表一起被删，重建
CREATE INDEX idx_grants_source ON block_grants(source);
