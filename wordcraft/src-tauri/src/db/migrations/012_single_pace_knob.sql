-- 学习量合并为单一旋钮：每日新词预算。
--
-- `daily_new_words` 原本是**每场**配额，后端在每个时段的 build() 里各读一次，
-- 三个时段就是三倍——设 14 实际是每天 42 个，界面上看不出来。语义改为
-- 每日预算后，旧值需要乘 3 才能保持用户当前的实际学习量不变。
--
-- `session_word_count` 不再是独立设置：单场题数由新词预算推算（见 plan.rs）。
-- 删掉这一行而不是留着不读——不读的设置键迟早会被重新接上。

UPDATE settings
   SET value = CAST(MIN(CAST(value AS INTEGER) * 3, 60) AS TEXT)
 WHERE key = 'daily_new_words'
   AND CAST(value AS INTEGER) > 0;

DELETE FROM settings WHERE key = 'session_word_count';
