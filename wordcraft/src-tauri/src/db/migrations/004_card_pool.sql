-- 卡池数据（contracts §10.2 卡池 B：原创像素生物）
--
-- 卡池随包分发、非用户数据，故走 migration 而非运行时导入。
-- 素材由 scripts/cards/generate_creatures.py 程序化生成，全部原创（§10.3）。
-- source 字段记录来源与许可证，spec F12 验收项要求可追溯。
--
-- 卡池 A（公有领域名画像素化）尚未纳入：需要下载 Wikimedia PD 原图，
-- 待素材就位后以新 migration 追加。

INSERT OR IGNORE INTO cards (id, name, card_type, rarity, image_path, trivia, source) VALUES
  (1, '草泥怪', 'creature', 1, '/cards/creatures/creature_01.png', '史莱姆类生物的身体含水量超过 90%，与水母相当。', '原创生成 · scripts/cards/generate_creatures.py · CC0'),
  (2, '水泡精', 'creature', 1, '/cards/creatures/creature_02.png', '水的表面张力能让小昆虫站在水面上行走。', '原创生成 · scripts/cards/generate_creatures.py · CC0'),
  (3, '火苗兽', 'creature', 1, '/cards/creatures/creature_03.png', '火焰的颜色取决于温度：红色约 800°C，蓝色可达 1400°C。', '原创生成 · scripts/cards/generate_creatures.py · CC0'),
  (4, '石背龟', 'creature', 1, '/cards/creatures/creature_04.png', '陆龟的甲壳由脊椎和肋骨演化融合而成，无法脱壳。', '原创生成 · scripts/cards/generate_creatures.py · CC0'),
  (5, '嫩芽兔', 'creature', 1, '/cards/creatures/creature_05.png', '兔子的视野接近 360 度，但正前方有盲区。', '原创生成 · scripts/cards/generate_creatures.py · CC0'),
  (6, '寒露蝶', 'creature', 1, '/cards/creatures/creature_06.png', '蝴蝶用脚上的感受器尝味道，而不是用口器。', '原创生成 · scripts/cards/generate_creatures.py · CC0'),
  (7, '电火虫', 'creature', 1, '/cards/creatures/creature_07.png', '萤火虫的发光效率接近 100%，几乎不产生热量。', '原创生成 · scripts/cards/generate_creatures.py · CC0'),
  (8, '熔岩泥', 'creature', 1, '/cards/creatures/creature_08.png', '熔岩的黏度可以相差百万倍，取决于二氧化硅含量。', '原创生成 · scripts/cards/generate_creatures.py · CC0'),
  (9, '霜翼蛾', 'creature', 2, '/cards/creatures/creature_09.png', '雪花有六重对称，源于水分子的氢键角度。', '原创生成 · scripts/cards/generate_creatures.py · CC0'),
  (10, '雷角鹿', 'creature', 2, '/cards/creatures/creature_10.png', '闪电通道温度可达太阳表面的五倍。', '原创生成 · scripts/cards/generate_creatures.py · CC0'),
  (11, '寒霜兽', 'creature', 2, '/cards/creatures/creature_11.png', '北极狐的皮毛能在零下 40 度保持体温不流失。', '原创生成 · scripts/cards/generate_creatures.py · CC0'),
  (12, '深潜灵', 'creature', 2, '/cards/creatures/creature_12.png', '海洋最深处的马里亚纳海沟压力超过一千个大气压。', '原创生成 · scripts/cards/generate_creatures.py · CC0'),
  (13, '岩翼龙', 'creature', 2, '/cards/creatures/creature_13.png', '翼龙并非恐龙，它们属于独立的爬行动物支系。', '原创生成 · scripts/cards/generate_creatures.py · CC0'),
  (14, '星辉晶', 'creature', 3, '/cards/creatures/creature_14.png', '石英晶体的压电效应是石英表走时精准的原理。', '原创生成 · scripts/cards/generate_creatures.py · CC0'),
  (15, '永冻核', 'creature', 3, '/cards/creatures/creature_15.png', '冰有至少 19 种晶体结构，日常见到的只是其中一种。', '原创生成 · scripts/cards/generate_creatures.py · CC0'),
  (16, '熔金石', 'creature', 3, '/cards/creatures/creature_16.png', '地球内核温度与太阳表面相当，约 5500°C。', '原创生成 · scripts/cards/generate_creatures.py · CC0');
