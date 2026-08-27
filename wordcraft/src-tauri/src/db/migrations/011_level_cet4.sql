-- 放宽 words.level 的受控词表，收入四级词（contracts §8）
--
-- 四级词单列一档而非并进 senior，是为了把选择权留给用户：高考前混进来会
-- 稀释重点，考完正好接着用。
--
-- 这张受控词表在四个地方各有一份——抽词脚本、build_library.py、
-- words.rs 的 VALID_LEVELS、这里的 CHECK。四级词导入时四处都要放行，
-- 而它们是被一次次导入失败逐个找出来的。
--
-- SQLite 改不了 CHECK，只能重建表。而 `words` 被 word_states / review_logs /
-- placement_words 引用，直接 DROP 会撞外键——空库上不会，有数据的库上必然。
--
-- 迁移执行器会在事务外关闭外键（见 migrations.rs::apply 的说明），
-- 否则 DROP 会触发 word_states 的 ON DELETE CASCADE，把学习状态一并删光。
-- 提交前有 foreign_key_check 兜底。

CREATE TABLE words_new (
  id              INTEGER PRIMARY KEY,
  word            TEXT    NOT NULL UNIQUE,
  phonetic        TEXT    NOT NULL DEFAULT '',
  pos             TEXT    NOT NULL,
  meaning         TEXT    NOT NULL,
  example_1       TEXT    NOT NULL,
  example_2       TEXT    NOT NULL DEFAULT '',
  level           TEXT    NOT NULL,
  frequency_band  INTEGER NOT NULL,
  zone            TEXT    NOT NULL,
  source_edition  TEXT    NOT NULL DEFAULT '',
  created_at      TEXT    NOT NULL,

  CHECK (level IN ('junior', 'senior', 'cet4', 'art')),
  CHECK (frequency_band BETWEEN 1 AND 5),
  CHECK (zone IN ('newbie', 'grass', 'water', 'fire', 'thunder', 'ice', 'rock'))
);

INSERT INTO words_new SELECT * FROM words;
DROP TABLE words;
ALTER TABLE words_new RENAME TO words;

-- 索引随旧表一起被删，重建
CREATE INDEX idx_words_zone_band ON words(zone, frequency_band);
CREATE INDEX idx_words_pos       ON words(pos);
