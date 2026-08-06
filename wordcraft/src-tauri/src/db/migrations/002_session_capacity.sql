-- 会话容量配置（决议 S13）
--
-- T08 实测：新词吞吐 ≈ 总词次 ÷ 9.3（每学 1 新词产生约 4.7 复习 + 3.6 强化词次）。
-- spec §3.1 的「每场 3-5 词」只能提供 1.62 新词/天，仅为 contracts §9.1 假设值的 27%。
-- 改为每场 20 词后达 5.78 新词/天，640 天覆盖 3699 词。
--
-- 此值可在设置中调整；前端不得硬编码，一律经 settings 读取。

INSERT OR IGNORE INTO settings (key, value) VALUES
  ('session_word_count', '20');
