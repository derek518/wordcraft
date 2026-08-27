-- 词频排名：能力模型的难度轴。
--
-- `frequency_band` 把 5278 个词压成 5 桶，一桶上千个词，无法区分 the（第 1 名）
-- 和排在第 900 名的词。能力模型需要连续的难度标尺才能推断「这个词孩子会不会」，
-- 所以把原始排名（BNC 与当代语料库中较高频的那个）单独存一列。
--
-- 可空：18 个连字符复合词（ice-cream / father-in-law / cd-rom）两个语料库都未
-- 收录。不插补——编一个排名会让能力模型把凭空捏造的难度当成证据。
--
-- 值由下一次词库导入回填（library.json 指纹已变，启动时自动重导）。

ALTER TABLE words ADD COLUMN frequency_rank INTEGER;

CREATE INDEX idx_words_rank ON words(frequency_rank);
