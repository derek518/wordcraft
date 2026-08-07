-- 卡池 A：公有领域名画像素化（contracts §10.2）
--
-- 每张的许可由 scripts/cards/fetch_paintings.py 经 Wikimedia API 核验，
-- LicenseShortName 不在 PD 白名单内的直接拒绝下载——作者逝世年份靠人记
-- 会出错，这个判断交给代码。
--
-- 原图不入库（§10.3），仅提交像素化成品。source 记录 Wikimedia 页面 URL，
-- spec F12 验收项要求逐张可追溯。

INSERT OR IGNORE INTO cards (id, name, card_type, rarity, image_path, trivia, source) VALUES
  (101, '星月夜', 'painting', 3, '/cards/paintings/painting_01.png', '梵高在圣雷米精神病院期间画下它，画中村庄是凭记忆虚构的。', 'Wikimedia Commons · Public domain · https://commons.wikimedia.org/wiki/File:Van_Gogh_-_Starry_Night_-_Google_Art_Project.jpg'),
  (102, '神奈川冲浪里', 'painting', 3, '/cards/paintings/painting_02.png', '画中远处的小山是富士山；这幅浮世绘曾影响德彪西创作交响诗《海》。', 'Wikimedia Commons · Public domain · https://commons.wikimedia.org/wiki/File:Great_Wave_off_Kanagawa2.jpg'),
  (103, '呐喊', 'painting', 3, '/cards/paintings/painting_03.png', '蒙克说灵感来自一次散步时「听见穿过自然的无尽呐喊」。', 'Wikimedia Commons · Public domain · https://commons.wikimedia.org/wiki/File:Edvard_Munch,_1893,_The_Scream,_oil,_tempera_and_pastel_on_cardboard,_91_x_73_cm,_National_Gallery_of_Norway.jpg'),
  (104, '戴珍珠耳环的少女', 'painting', 2, '/cards/paintings/painting_04.png', '研究认为那颗「珍珠」可能只是抛光的锡，真珍珠不会有这么大。', 'Wikimedia Commons · Public domain · https://commons.wikimedia.org/wiki/File:1665_Girl_with_a_Pearl_Earring.jpg'),
  (105, '向日葵', 'painting', 2, '/cards/paintings/painting_05.png', '梵高画了至少 11 幅向日葵，用来装饰高更来访时的房间。', 'Wikimedia Commons · Public domain · https://commons.wikimedia.org/wiki/File:Vincent_Willem_van_Gogh_127.jpg'),
  (106, '大碗岛的星期天下午', 'painting', 2, '/cards/paintings/painting_06.png', '修拉用点彩法画了两年，颜色由观者的眼睛在视网膜上混合。', 'Wikimedia Commons · Public domain · https://commons.wikimedia.org/wiki/File:A_Sunday_on_La_Grande_Jatte,_Georges_Seurat,_1884.jpg'),
  (107, '睡莲', 'painting', 1, '/cards/paintings/painting_07.png', '莫奈晚年患白内障，视野偏黄，这改变了他后期作品的色调。', 'Wikimedia Commons · Public domain · https://commons.wikimedia.org/wiki/File:Claude_Monet_-_Water_Lilies_-_1906,_Ryerson.jpg'),
  (108, '拾穗者', 'painting', 1, '/cards/paintings/painting_08.png', '拾穗是当时法律赋予穷人的权利：允许在收割后捡拾遗落的麦穗。', 'Wikimedia Commons · Public domain · https://commons.wikimedia.org/wiki/File:Jean-Fran%C3%A7ois_Millet_(II)_013.jpg');
