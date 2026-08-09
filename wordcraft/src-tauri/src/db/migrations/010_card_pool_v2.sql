-- 卡池 v2：遗忘之境守护者体系
--
-- 旧卡池 16 生物 + 8 画作，替换为 42 张按元素分组的卡：
-- 碎片 / 生物 / 器物（普通）· 守护者 / 神器（稀有）· 守护者（传说）。
--
-- 三处必须按顺序做，否则整条迁移失败：
--
-- 1. **先清引用方再清 cards**。`card_collection` 与 `homestead_residents`
--    都有 `REFERENCES cards(id)`，反过来删会撞外键约束——应用起不来。
-- 2. **card_type 的 CHECK 要放宽**。旧约束只允许 painting / creature，
--    新卡池有 shard / item / guardian / artifact。SQLite 改不了 CHECK，只能重建表。
-- 3. **旧收集记录只能清空，不能保留**。卡 id 被整体复用（旧 4 号是石背龟，
--    新 4 号是草药袋），留着会把用户抽到的卡悄悄换成另一张。

-- 引用方先清空，顺序不能反
DELETE FROM homestead_residents;
DELETE FROM card_collection;

-- 重建 cards，放宽 card_type。此时 cards 已无引用方持有行，可安全 DROP
CREATE TABLE cards_new (
  id          INTEGER PRIMARY KEY,
  name        TEXT    NOT NULL,
  card_type   TEXT    NOT NULL,
  rarity      INTEGER NOT NULL,
  image_path  TEXT    NOT NULL,
  trivia      TEXT    NOT NULL DEFAULT '',
  -- spec F12 验收项：素材来源与许可证必须可追溯
  source      TEXT    NOT NULL,

  CHECK (card_type IN ('shard', 'creature', 'item', 'guardian', 'artifact')),
  CHECK (rarity BETWEEN 1 AND 3)
);

DROP TABLE cards;
ALTER TABLE cards_new RENAME TO cards;

INSERT INTO cards (id, name, card_type, rarity, image_path, trivia, source) VALUES
-- ========== 普通卡 (N) 24张 ==========
(1, '翠叶碎片', 'shard', 1, '/assets/cards/common/grass_leaf_shard.png', '清风平原最常见的植物碎片，蕴含着微弱的草元素能量。', '原创生成 · generate_cards.py · CC0'),
(2, '芽苗精', 'creature', 1, '/assets/cards/common/grass_sprout.png', '刚从土里探出头的小精灵，对世界充满好奇。', '原创生成 · generate_cards.py · CC0'),
(3, '藤虫', 'creature', 1, '/assets/cards/common/grass_vine_bug.png', '以藤蔓为家的昆虫，移动时会留下发光的足迹。', '原创生成 · generate_cards.py · CC0'),
(4, '草药袋', 'item', 1, '/assets/cards/common/grass_herb_pouch.png', '冒险者常备的药草包，能治愈轻微的伤口。', '原创生成 · generate_cards.py · CC0'),
(5, '水珠碎片', 'shard', 1, '/assets/cards/common/water_water_drop.png', '蓝水湖泊凝结的水珠，触摸时会有清凉的感觉。', '原创生成 · generate_cards.py · CC0'),
(6, '泡泡鱼', 'creature', 1, '/assets/cards/common/water_bubble_fish.png', '喜欢吐泡泡的小鱼，泡泡里倒映着湖底的景象。', '原创生成 · generate_cards.py · CC0'),
(7, '水母仔', 'creature', 1, '/assets/cards/common/water_jelly_baby.png', '刚孵化的小水母，触须还不具备麻痹能力。', '原创生成 · generate_cards.py · CC0'),
(8, '海螺壳', 'item', 1, '/assets/cards/common/water_conch_shell.png', '深海螺壳，放在耳边能听到海浪的声音。', '原创生成 · generate_cards.py · CC0'),
(9, '火炭碎片', 'shard', 1, '/assets/cards/common/fire_ember_shard.png', '赤焰山脉的余烬，即使离开火山也不会熄灭。', '原创生成 · generate_cards.py · CC0'),
(10, '小火苗', 'creature', 1, '/assets/cards/common/fire_small_flame.png', '一团有自我意识的小火焰，喜欢追逐飞蛾。', '原创生成 · generate_cards.py · CC0'),
(11, '熔岩虫', 'creature', 1, '/assets/cards/common/fire_lava_worm.png', '生活在岩浆边缘的蠕虫，外壳能抵御高温。', '原创生成 · generate_cards.py · CC0'),
(12, '火把', 'item', 1, '/assets/cards/common/fire_torch.png', '永不熄灭的火把，照亮遗忘之境的黑暗角落。', '原创生成 · generate_cards.py · CC0'),
(13, '电光碎片', 'shard', 1, '/assets/cards/common/thunder_spark_shard.png', '雷霆峡谷收集的静电结晶，触碰会发麻。', '原创生成 · generate_cards.py · CC0'),
(14, '静电虫', 'creature', 1, '/assets/cards/common/thunder_static_bug.png', '翅膀摩擦会产生静电的小虫，雨天特别活跃。', '原创生成 · generate_cards.py · CC0'),
(15, '雷云仔', 'creature', 1, '/assets/cards/common/thunder_cloud_baby.png', '一团有表情的小雷云，心情不好会打雷。', '原创生成 · generate_cards.py · CC0'),
(16, '电池', 'item', 1, '/assets/cards/common/thunder_battery.png', '储存雷电能量的装置，是机械装置的能源核心。', '原创生成 · generate_cards.py · CC0'),
(17, '冰晶碎片', 'shard', 1, '/assets/cards/common/ice_ice_shard.png', '永冬之巅凝结的冰晶，内部封存着古老的记忆。', '原创生成 · generate_cards.py · CC0'),
(18, '小雪球', 'creature', 1, '/assets/cards/common/ice_snow_ball.png', '会自己滚动的小雪球，滚过的地方会留下霜花。', '原创生成 · generate_cards.py · CC0'),
(19, '霜虫', 'creature', 1, '/assets/cards/common/ice_frost_bug.png', '在冰层下冬眠的虫子，春天会破冰而出。', '原创生成 · generate_cards.py · CC0'),
(20, '冰锥', 'item', 1, '/assets/cards/common/ice_ice_spike.png', '锋利的冰锥，据说能刺穿遗忘魔王的铠甲。', '原创生成 · generate_cards.py · CC0'),
(21, '砂石碎片', 'shard', 1, '/assets/cards/common/rock_sand_shard.png', '金石荒漠的砂石，每一粒都记载着大地的历史。', '原创生成 · generate_cards.py · CC0'),
(22, '岩甲虫', 'creature', 1, '/assets/cards/common/rock_rock_beetle.png', '外壳如岩石般坚硬的小虫，天敌很少。', '原创生成 · generate_cards.py · CC0'),
(23, '矿工鼠', 'creature', 1, '/assets/cards/common/rock_miner_rat.png', '擅长挖掘地道的鼠类，能找到深埋的宝石。', '原创生成 · generate_cards.py · CC0'),
(24, '矿镐', 'item', 1, '/assets/cards/common/rock_pickaxe.png', '矮人锻造的矿镐，据说能敲开任何岩石。', '原创生成 · generate_cards.py · CC0'),

-- ========== 稀有卡 (R) 12张 ==========
(25, '荆棘守卫', 'guardian', 2, '/assets/cards/rare/grass_thorn_guard.png', '清风平原的精英战士，荆棘铠甲让敌人无法靠近。', '原创生成 · generate_cards.py · CC0'),
(26, '生命之种', 'artifact', 2, '/assets/cards/rare/grass_life_seed.png', '传说中能复活枯萎森林的神圣种子。', '原创生成 · generate_cards.py · CC0'),
(27, '潮汐使者', 'guardian', 2, '/assets/cards/rare/water_tide_herald.png', '蓝水湖泊的守护者，能操控潮汐的涨落。', '原创生成 · generate_cards.py · CC0'),
(28, '深海珍珠', 'artifact', 2, '/assets/cards/rare/water_deep_pearl.png', '千年深海蚌孕育的珍珠，据说能实现一个愿望。', '原创生成 · generate_cards.py · CC0'),
(29, '烈焰骑士', 'guardian', 2, '/assets/cards/rare/fire_flame_knight.png', '赤焰山脉的勇士，火焰长剑能斩断一切黑暗。', '原创生成 · generate_cards.py · CC0'),
(30, '熔岩之心', 'artifact', 2, '/assets/cards/rare/fire_magma_heart.png', '火山核心凝结的宝石，持有者永不感到寒冷。', '原创生成 · generate_cards.py · CC0'),
(31, '风暴之眼', 'guardian', 2, '/assets/cards/rare/thunder_storm_eye.png', '雷霆峡谷的观察者，双眼能看穿一切伪装。', '原创生成 · generate_cards.py · CC0'),
(32, '雷霆之锤', 'artifact', 2, '/assets/cards/rare/thunder_thunder_hammer.png', '矮人王锻造的战锤，一击能粉碎山峰。', '原创生成 · generate_cards.py · CC0'),
(33, '霜冻卫士', 'guardian', 2, '/assets/cards/rare/ice_frost_warden.png', '永冬之巅的守卫，冰甲上刻满了古老的符文。', '原创生成 · generate_cards.py · CC0'),
(34, '永冬之镜', 'artifact', 2, '/assets/cards/rare/ice_eternal_mirror.png', '能映照过去的魔镜，但看到的未必是真相。', '原创生成 · generate_cards.py · CC0'),
(35, '山岭巨人', 'guardian', 2, '/assets/cards/rare/rock_ridge_giant.png', '金石荒漠的远古居民，一步能跨越整座山谷。', '原创生成 · generate_cards.py · CC0'),
(36, '金石护盾', 'artifact', 2, '/assets/cards/rare/rock_gold_shield.png', '用最坚硬的岩石打造的盾牌，能抵挡任何攻击。', '原创生成 · generate_cards.py · CC0'),

-- ========== 传说卡 (SR) 6张 ==========
(37, '翠灵龙', 'guardian', 3, '/assets/cards/legend/grass_guardian.png', '清风平原的至高守护者，呼出的气息能让荒漠变绿洲。', '原创生成 · generate_cards.py · CC0'),
(38, '潮汐鲸', 'guardian', 3, '/assets/cards/legend/water_guardian.png', '蓝水湖泊的主宰，游过时整个湖面都会泛起涟漪。', '原创生成 · generate_cards.py · CC0'),
(39, '炎凤', 'guardian', 3, '/assets/cards/legend/fire_guardian.png', '赤焰山脉的不死鸟，每次重生都会变得更加强大。', '原创生成 · generate_cards.py · CC0'),
(40, '雷鹰', 'guardian', 3, '/assets/cards/legend/thunder_guardian.png', '雷霆峡谷的霸主，双翼展开时天空会为之变色。', '原创生成 · generate_cards.py · CC0'),
(41, '霜狼', 'guardian', 3, '/assets/cards/legend/ice_guardian.png', '永冬之巅的孤独王者，足迹所至皆为冰封。', '原创生成 · generate_cards.py · CC0'),
(42, '岩龟', 'guardian', 3, '/assets/cards/legend/rock_guardian.png', '金石荒漠的远古存在，背上的甲壳记载着世界的历史。', '原创生成 · generate_cards.py · CC0');
