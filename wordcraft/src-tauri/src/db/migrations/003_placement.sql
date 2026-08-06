-- 摸底分级进度（contracts §9.2）
--
-- 契约允许摸底分两次完成，故每层的作答统计必须落库而非留在前端内存——
-- 中途关闭应用后重开，已答的题不该重来。
--
-- 只记每层的聚合结果，不记逐题明细：§9.2② 的产出是「每层掌握率 p₁..p₅」，
-- 60 题覆盖不了 1600 词，逐词判定本就不是目标。

CREATE TABLE placement_results (
  band            INTEGER PRIMARY KEY,
  asked           INTEGER NOT NULL DEFAULT 0,
  -- 「已会」的判定含防猜条件（§9.3）：答对且反应 < 4000ms。
  -- 只数答对会把四选一 25% 的基础猜对率算成掌握率
  passed          INTEGER NOT NULL DEFAULT 0,
  -- 该层是否已测完（题数满或连续错触发提前结束）
  is_closed       INTEGER NOT NULL DEFAULT 0,
  -- 当前连续答错数。存库而非留在前端：连错几次才收束是产品规则，
  -- 前端不该知道这个数字，否则规则改动要两边同时改
  consecutive_miss INTEGER NOT NULL DEFAULT 0,

  CHECK (band BETWEEN 1 AND 5),
  CHECK (asked >= 0),
  CHECK (passed >= 0 AND passed <= asked),
  CHECK (is_closed IN (0, 1)),
  CHECK (consecutive_miss >= 0)
);

-- 已出过的题，避免同一次摸底里重复出现同一个词
CREATE TABLE placement_asked (
  word_id     INTEGER PRIMARY KEY REFERENCES words(id) ON DELETE CASCADE,
  asked_at    TEXT NOT NULL
);
