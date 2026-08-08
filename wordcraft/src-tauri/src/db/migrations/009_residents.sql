-- 家园居民。收集到的生物卡可以住进建成的聚落。
--
-- 抽卡此前只产出图鉴里的一张图；家园此前没有任何活物。这张表把两者接起来：
-- 建成蓝图解锁入住位，卡牌有了去处，家园有了住户。

CREATE TABLE homestead_residents (
  slot        INTEGER PRIMARY KEY,
  card_id     INTEGER NOT NULL REFERENCES cards(id),
  moved_in_at TEXT    NOT NULL,

  -- 同一只生物不能同时住两个位置。没有这条约束，用户会把唯一一张
  -- 稀有卡填满所有位置，收集的意义随之消失
  UNIQUE (card_id)
);
