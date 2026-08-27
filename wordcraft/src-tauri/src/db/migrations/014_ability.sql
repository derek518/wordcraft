-- 能力估计的持久化。
--
-- θ 是孩子的词汇掌握边界在词频轴上的位置，由**每次首见作答**更新
-- （见 src/ability.rs）。它取代了「初中 / 高中 / 四级」这种手选范围：
-- 那三个标签和难度基本无关，用它们筛选既在练已经会的词，又在漏掉该练的词。
--
-- 三列可空 / 默认 0 表示「还没有任何观测」，加载时回落到 ability::PRIOR_*。
-- 不把先验写进 DEFAULT：那会在 SQL 里留一份常量副本，改了 Rust 侧就分叉。
--
-- `ability_information` 是累计 Fisher 信息量，决定单次观测的步长——
-- 前几次大幅修正，几十次后自然稳定。

ALTER TABLE player_stats ADD COLUMN ability_theta REAL;
ALTER TABLE player_stats ADD COLUMN ability_information REAL NOT NULL DEFAULT 0;
ALTER TABLE player_stats ADD COLUMN ability_observations INTEGER NOT NULL DEFAULT 0;
