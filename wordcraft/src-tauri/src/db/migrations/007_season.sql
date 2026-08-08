-- 赛季赛道（spec §4.2 F11）
--
-- 只记结算历史，不记进度——本周完成了几个时段可以从 sessions 实时聚合，
-- 再存一份就有了两个真相来源，迟早对不上。

CREATE TABLE season_settlements (
  -- 该周周一的日期。用日期而非 ISO 周数：跨年时 W53 与次年 W01
  -- 会相邻甚至重叠，日期则天然唯一有序
  week_start    TEXT    PRIMARY KEY,
  sessions_done INTEGER NOT NULL,
  points_earned INTEGER NOT NULL,
  settled_at    TEXT    NOT NULL,

  CHECK (sessions_done >= 0),
  CHECK (points_earned >= 0)
);

-- 赛道积分。spec F11 明确「断签不清赛道积分（只清 streak）」——
-- 积分是已经付出的努力，不该被一次中断抹掉
ALTER TABLE player_stats ADD COLUMN track_points INTEGER NOT NULL DEFAULT 0;
