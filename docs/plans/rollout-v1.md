# WordCraft V1.0 实施计划（契约 plan）

> 目标：把当前「能编译、核心功能为零的 UI 外壳」推进到 spec **M1 MVP 验收线**——可安装、弹窗正常、间隔重复真实工作、数据持久化。
> 契约见 [contracts-v1.md](./contracts-v1.md)。审计依据见 [../audit-2026-08-05.md](../audit-2026-08-05.md)。
> 本文件只写**约束与验收**，不写实现代码。

## 命名说明

本文用 **Phase 0–6** 表示实施阶段，避免与 spec 的 M1/M2/M3 里程碑混淆。
**Phase 0–6 全部完成 = spec M1 MVP 达标**。spec M2/M3（F8–F13）属 V1.1+，不在本计划范围。

---

## 依赖图与并行性

```
Phase 0 (地基)  ──> Phase 1 (数据层) ──┬──> Phase 2 (FSRS 引擎) ──┐
                                       │                          ├──> Phase 4 (题型) ──> Phase 6 (摸底+打磨)
                                       └──> Phase 3 (词库管线) ───┘                            ▲
                                                                                               │
                     Phase 5 (弹窗/系统集成) ← 依赖 Phase 1，可与 2/3/4 并行 ───────────────────┘
```

- **Phase 2 与 Phase 3 可并行**（FSRS 不依赖真实词库，词库管线不依赖算法）
- **Phase 5 可在 Phase 1 完成后随时启动**，与 2/3/4 并行；它是唯一需要 Windows 真机验证的阶段
- Phase 4 需要 Phase 2（状态机）+ Phase 3（真词库，干扰项依赖同词性候选池）

---

# Phase 0 · 地基修复

> 阻塞一切后续工作。审计 D3（破坏性脚本）与工程规范缺口在此清偿。

### T01 · 建立版本控制
- 在 `wordcraft/` 执行 `git init`，当前状态作为 baseline 首次提交
- 修正 `.gitignore`：确保排除 `src-tauri/target/`、`dist/`、`node_modules/`、`.DS_Store`
- **验证**：`git status` 干净；`du -sh .git` 合理（不含 2G target）
- **DoD**：可回滚到任意历史点

### T02 · 清除死代码与破坏性脚本
- 删除 `src-tauri/fix_db.py`（审计 D3：把 chrono 改成错误手写实现的脚本）
- 删除 `src-tauri/src/lib.rs`（Tauri 模板残留，`run()` 从未被调用），同步移除 `Cargo.toml` 的 `[lib]` 段
- 删除 `src/App.css`（184 行模板样式，零引用）、`src/assets/react.svg`、`src/assets/vite.svg`、`src/assets/hero.png`
- 用真实项目说明重写 `README.md`（现为 Vite 模板文案）
- **验证**：`cargo check` + `npx tsc -b` 通过；`grep -rn "App.css\|hero.png" src/` 无结果
- **DoD**：仓库中不存在任何未被引用的模板残留

### T03 · 依赖对齐
- Rust 新增：`chrono`（ADR-4）、`rusqlite`(bundled) 或 `tauri-plugin-sql`、`tauri-plugin-autostart`、`tauri` 开启 `tray-icon` feature
- Rust 新增（仅 Windows target）：`windows` crate，features 覆盖 `Win32_UI_Shell`（`SHQueryUserNotificationState`）
- 前端确认 `ts-fsrs` 已装（package.json 已有，但**当前零处引用**）；新增 `vitest`
- **验证**：`cargo tree | grep chrono` 有结果；macOS 上 `cargo check` 通过（Windows-only 依赖需正确置于 `[target.'cfg(windows)'.dependencies]`）
- **DoD**：跨平台依赖声明正确，macOS 与 Windows 均可 `cargo check`

### T04 · 建立工程规范文档
- 新建 `wordcraft/CLAUDE.md`：架构说明、常用命令、目录约定、5 层 DoD、指向 contracts-v1.md
- 新建 `wordcraft/MOCKS.md`：mock 库存清单，初始登记 Phase 5 之前允许存在的平台 stub
- **DoD**：spec §6「仓库根目录提供 CLAUDE.md」项达标

### T05 · 测试骨架
- Rust：在 `src-tauri/src/` 各模块建立 `#[cfg(test)]` 结构，配置 sqlite in-memory 测试夹具
- 前端：安装配置 `vitest`，`package.json` 增加 `test` / `test:coverage` 脚本
- 各写 1 个冒烟测试证明管线通
- **验证**：`cargo test` 与 `npm test` 均能跑出 PASS
- **DoD**：后续任务可以直接写测试，无需再搭环境

**Phase 0 出口条件**：`cargo clippy` 零警告 · `npx oxlint` 零警告 · `cargo test` / `npm test` 可运行 · git 历史存在

---

# Phase 1 · 数据层重建（SQLite）

> 对应审计 D1/D2/D4。当前 JSON 存储与错误日期函数在此彻底替换。

### T06 · Migration 引擎 + migration 001
- 新建 `src-tauri/src/db/migrations.rs`，实现版本化迁移（读写 `schema_migrations` 表）
- 落地 contracts-v1.md §2 全部 DDL
- **旧数据处理**：开发期 JSON 数据（52 词 + 少量 log）**直接放弃**，不写迁移脚本；启动时若检测到旧 `wordcraft_data.json` 则重命名为 `.bak` 并记 info 日志
- **测试断言**：空库跑完 001 后 `PRAGMA table_info` 与 §2 逐列一致；重复执行幂等
- **DoD**：schema-vs-DB drift gate 通过（实跑 PRAGMA 校验，非只看 ledger）

### T07 · Repository 层
- 新建 `src-tauri/src/db/repo/` 下 `words.rs` / `word_states.rs` / `review_logs.rs` / `sessions.rs` / `player_stats.rs` / `settings.rs`
- 全部时间处理走 `chrono`；UTC 存储 + 本地日归属（ADR-5）
- 单文件 200–400 行上限，超出即拆
- **测试断言**：跨本地午夜的两条 log 被分入不同 `date`
- **DoD**：`grep -rn "86400\|86464\|SystemTime::now" src-tauri/src/` 除 chrono 内部外无手写日期运算

### T08 · 排队算法 `get_session_queue` + 自适应控制
- 实现 contracts §3.1 + §4.1 **三档自适应**（新词额度与强化配额随强化池大小 R 调整）
- 优先级：`reinforcing` > 到期复习(`due_at <= now`) > **摸底词抽查（仅填充剩余空位）** > 新词
- 合并逻辑：上一时段未完成时并入本时段，总量封顶 8（spec F1）
- 自适应机制**不做 UI 暴露**（避免「系统在惩罚我」的感受）
- **测试断言**：R=15/16/30/31 四个边界的额度与配额取值；R 回落后自动恢复；强化池空时不报错；`due_at > now` 不出现
- **DoD**：审计 D1「答过的词永不重现」被证伪——同一词在第二次调用时按 due_at 正确重现。**用 180 天模拟脚本回归验证强化池收敛**（决议 S3）

### T09 · `commit_review` 事务
- 单事务同时写 `word_states` + `review_logs`（contracts §3.2）
- 校验载荷：`rating` ∈ 1..4、`questionType` ∈ 1..5、`appState` ∈ 受控值，非法即 Err
- **测试断言**：中途注入错误后两张表都不变（原子性）
- **DoD**：无静默失败路径，全部错误可诊断

### T10 · 统计查询
- 实现 contracts §3.4 五个命令 + `export_data_json`
- **测试断言**：`get_mastery_distribution` 五段之和等于 `words` 总数
- **DoD**：`StatsPanel` 所需字段全部由真实 SQL 聚合产出

**Phase 1 出口条件（集成门禁）**
- 实际启动 `npm run tauri dev`，在 UI 中完成 ≥5 次作答
- 用 `sqlite3` 直接打开 DB 文件，**眼睛确认** `review_logs` 有对应行、`word_states.due_at` 随 rating 变化
- 重启 App，数据仍在且到期词正确重现
- ❌ 不接受「单元测试全过」作为本阶段完成标志

---

# Phase 2 · FSRS 引擎（前端）

> ADR-2：ts-fsrs 在前端。当前 `fsrs_engine.rs` 的 83 行与 FSRS 无关，将被删除。

### T11 · ts-fsrs 适配层
- 新建 `src/core/fsrs.ts`：封装 ts-fsrs 的 `Scheduler`，输入当前 `word_states` 行 + rating，输出 contracts §3.2 的 `after` 结构
- 参数用 ts-fsrs 默认权重起步（spec §5）
- **测试断言**：同一 (state, rating) 下 ts-fsrs 输出被完整映射，无字段丢失
- **DoD**：删除 `src-tauri/src/fsrs_engine.rs`

### T12 · 自动评级映射（按题型分阈值）
- 新建 `src/core/rating.ts`，实现 contracts §5 的 **FAST/SLOW 题型阈值表**（纯函数）
- **测试断言**：每个题型各自的边界值（Lv.1 的 2999/3000/7999/8000、Lv.5 的 7999/8000/19999/20000 等）；Lv≥4 答对上调且封顶 Easy
- **DoD**：无 Anki 式自评 UI（spec F2 禁止）；决议 S5 关闭——拼写题不再因打字耗时被误判 Hard

### T13 · 业务状态机
- 新建 `src/core/stateMachine.ts`，实现 contracts §4 转移表（纯函数）
- **测试断言**：转移表每一行；`reinforcing` 连对 1 次不升级、**第 2 次升级**（决议 S3）；8s 外答对清零；mastered 抽查失败回落且清 `mastered_at`
- **DoD**：强化队列离队条件与 contracts §4 一致

### T14 · XP / 等级 / Streak（新规则）
- 新建 `src/core/progression.ts`，实现 contracts §7 + **§7.1 新 streak 判定**
- Streak 五条分支：暂停冻结 / `eligible_sessions=0` 冻结 / ≥2 完成 +1 / 3 完成 perfect / 断签消耗补签卡
- 实现 `daily_records` 写入、补签卡月度发放（`last_grant_month` 防重发）、`pause_used_month` 跨月重置
- 重写 `StatsPanel.tsx` 等级进度条（当前 `total_xp % 100` 与公式不符）
- **测试断言**：五条分支各一例；补签卡每月只发一次；连击倍率三档边界；等级公式在 0/50/200/500000 处取值
- **DoD**：决议 S1/S4/S6/S8 全部关闭——完成 2/3 真实 +1，全屏跳过日不断签，暂停日冻结

---

# Phase 3 · 词库管线（可与 Phase 2 并行）

> 决策：公开词库拿结构化字段 + AI 补例句。这是项目最大单项工作量。

### T15 · 公开词库获取与清洗（人教版 + 外研版融合）
- 用 `gh search repos` / `gh search code` 找 **人教版与外研版** 中考/高考词表（优先带音标/词性/释义的结构化 JSON）
- **融合策略**：两版取并集去重；`source_edition` 字段标注来源（`renjiao` / `waiyan` / `both`）；释义冲突时取并集合并
- 落地 `scripts/wordlist/raw/`，来源与许可证记入 `scripts/wordlist/SOURCES.md`
- 清洗：词性规范化到受控词表；`frequency_band` 按公开词频表标注（**不得手填**）
- 🔴 **强制验证项（决议 S2）**：`SELECT COUNT(DISTINCT word)` —— spec 假设 1600+3500 去重后仍为 5100，但高考考纲 3500 词表**通常已含**中考 1600 词。若实际仅 3500–3800，须回写 contracts §9.1 并把 `daily_new_words` 默认值下调至 4
- **验证**：抽样 30 词人工核对音标/释义（AI 自查不可靠，需真人参与）
- **DoD**：`frequency_band` 有可追溯依据；真实词表规模已确认并回写契约

### T16 · 分区映射
- 按 contracts §8「分区规则」由 `level + frequency_band` 推导 `zone`
- **验证**：新手村恰好 50 词且**均为高频基础词**（审计 D7：当前 52 词全是 a 开头）
- **DoD**：各 zone 词数与 spec §5.2 表格一致（50/200/300/500/500/500）

### T17 · AI 例句生成管线
- 新建 `scripts/wordlist/gen-examples.ts`：分批调用 Claude API 为每词生成 2 条例句
- 语境约束：Minecraft / 原神风格 / 赛车 / 绘画创作，**禁止出现受版权保护的角色名与专有名词**（spec §4 素材策略）
- 断点续传（按 word 记录已完成），失败重试，产出增量写盘
- **验证**：抽样 50 词人工审阅语境与语法；`grep` 检查无商业游戏专有名词
- **DoD**：全部词条 `example_1` 非空且包含该词的某个词形

### T18 · 导入校验与执行
- 实现 contracts §8「导入校验」全部规则，**失败即拒并报告，禁止静默跳过**
- 通过 `import_words` 批量导入，作为首次启动的内置数据
- 删除 `src/data/words.ts`（52 词硬编码）
- **验证**：`COUNT(*)` 与 T15 确认的规模一致；校验失败样本被正确拒绝并列出原因
- **DoD**：审计 D7 关闭

### T19 · TTS 全量预生成与缓存
- 实现 `prefetch_audio`：Edge-TTS 批量生成 mp3 到 `app_data_dir/audio_cache/`
- **策略（已确认目标机常联网，但仍全量预生成随包分发）**：spec F4 要求点击后 300ms 内播放，实时合成无法保证；联网仅用于增量补全与更新
- `play_word_audio` 缓存未命中时按 `tts_provider` 降级到 SAPI（Windows）
- **验证**：实际播放 10 个词并**听到声音**；点击到出声 <300ms（spec F4 验收）
- **DoD**：`tts.rs` 当前「建目录然后 Ok(())」的假实现被替换

---

# Phase 4 · 题型体系

### T20 · 干扰项生成
- 新建 `src/core/distractors.ts`，实现 contracts §6「干扰项来源」+「硬约束」
- Rust 侧提供 `get_distractor_pool`（同 `pos` 候选，走 `idx_words_pos`）
- 编辑距离用成熟库（先查 npm，勿手写）
- **测试断言**：4 选项互不相同；无子串包含；候选不足时降级路径被走到且仍返回 4 项
- **DoD**：`WordTrainer.tsx` 中 50 个硬编码释义数组被删除（审计 D5）

### T21 · 前端数据层重写
- `WordTrainer.tsx` 改用 `get_session_queue` + `commit_review`
- **删除 catch 中的本地 fallback**（审计 D6 silent failure）；后端失败时显示明确错误态
- `AdventureMap.tsx` 的 `completedToday` 接入 `get_today_sessions`（当前是永远为空的 useState）
- 修复 `useEffect` 缺失依赖告警
- **DoD**：断开后端（故意让命令 Err）时 UI 显示错误而非假数据

### T22 · 题型 Lv.1–Lv.5
- 按 contracts §6 实现五种题型组件，按 `question_level` 路由
- Lv.3 听音辨词接 T19 音频；Lv.5 拼写题带首字母提示
- **Lv.5 准入限制（决议 S10）**：仅 `frequency_band <= 2` 的核心词启用拼写，其余词最高阶止于 Lv.4
- **验证**：同一词在 learning 与 review 阶段呈现不同题型（spec F3 验收项）；band 3–5 的词永不出现拼写题
- **DoD**：五种题型均可在自由练习中触发并正确提交 `question_type`

---

# Phase 5 · 弹窗调度与系统集成（可与 2/3/4 并行）

> 唯一需要 Windows 真机验证的阶段。审计中 F1/F7 完成度为 0%。

### T23 · 平台抽象层
- 新建 `src-tauri/src/platform/{mod.rs, windows.rs, stub.rs}`，实现 contracts §3.5 的 trait
- stub 实现**必须返回 `BusyState::Unknown` 并记 warn**，禁止假装 Normal；登记进 `MOCKS.md`
- **DoD**：macOS 上可编译可运行，且日志明确提示「平台能力不可用」

### T24 · Windows 全屏/忙检测
- `windows.rs` 调用 `SHQueryUserNotificationState`，映射到 `BusyState`
- **验证（Windows 真机）**：开全屏 D3D 游戏 → 返回 `FullScreenD3D`；退出 → 返回 `Normal`
- **DoD**：spec F1 验收项「全屏游戏运行期间 0 次弹出」有真机证据

### T25 · 调度器
- 重写 `scheduler.rs`（当前是 `sleep(60)` 死循环 + 硬编码 `"09:00"`）
- 时段内每 5 分钟轮询；忙则等待，退出全屏后延迟 60 秒弹出
- 延后逻辑接 `postpone_session`（每时段 3 次上限）；未完成并入下一时段
- 移除 `static mut` + `unsafe`（改用 `tauri::State` 或 `OnceLock`）
- **测试断言**：给定虚拟时钟，时段边界/延后耗尽/合并三条路径的决策正确
- **DoD**：`get_next_session_time` 返回真实计算值（当前硬编码 60 分钟）

### T26 · 弹窗窗口
- `tauri.conf.json` 新增 popup 窗口：360×480、无边框、`alwaysOnTop`、`focus: false`、右下角定位
- 点击后才捕获键盘焦点（spec F1）
- **验证（Windows 真机）**：弹窗不抢焦点（游戏/打字不被打断）
- **DoD**：spec F1 窗口规格逐项达标

### T27 · 托盘与开机自启
- 托盘常驻，图标显示今日进度 0–3；菜单：立即练一组 / 打开主界面 / 今日暂停 / 冒险者手册
- `tauri-plugin-autostart` 接管注册表 Run 键，设置中可关闭
- **验证（Windows 真机）**：重启系统后自动启动；托盘图标随完成数变化
- **DoD**：spec F7 全项达标

---

# Phase 6 · 摸底测试、抽卡与收尾

### T28 · 摸底测试与批量预分级（contracts §9）
- **粗筛**：`level='senior'` 不进摸底（新高一大概率未学）；`level='art'` 锁定；摸底范围仅 junior ~1600 词
- **自适应测试**：5 层各约 12 题（共 ~60 题 / 5 分钟，可分两次）；连续 3 错或超时下跳一层
- **产出层掌握率 p₁..p₅**（非逐词判定）+ `vocab_estimate`
- **批量预分级**：按层掌握率赋 `app_state` 与 stability，**带 band 分层抖动**（band1-2: 7–30 天、band3-4: 30–90、band5: 90–180）——防止集中到期淹没日预算
- **防猜（决议 S7）**：判「已会」需答对 **且** `reaction_ms < 4000`；stability 起始 7–180 天而非固定 30
- 包装为「新手战力测试」
- **测试断言**：已会词不进新词队列，但 180 天内至少被排队抽查一次；预分级后任意单日到期数不超过日词次预算
- **DoD**：决议 S2/S7 关闭——摸底真实压缩待学量且假阳性可自动纠正

### T29 · 极简抽卡（决议 S9，MVP 留存钩子）
- 卡池素材：公有领域名画像素化（Wikimedia PD）+ 原创像素生物；处理脚本置 `scripts/cards/`
- 实现 contracts §10：完成时段发券、开卡动画、图鉴页（已收集/剪影）、重复卡计数
- 每张卡 `source` 字段记录来源 URL 与许可证 → 生成 `scripts/cards/SOURCES.md`
- **测试断言**：券不足时 `draw_card` 返回 Err 而非静默失败；稀有度分布 10000 次抽样符合 70/25/5（±2%）
- **DoD**：spec F12 验收项「卡面素材全部为公有领域或原创，仓库内附来源清单」达标

### T30 · 首启流程与设置页
- 首启：欢迎 → 摸底 → 新手村开放（重写 `App.tsx` 的 `checkFirstRun`）
- 设置页：时段、每日新词量、音量、开机自启、TTS 提供方
- **DoD**：全部设置项真实写入 `settings` 表并生效

### T31 · 打包与真机验收
- Windows 上 `npm run tauri build` 产出 `.msi`
- 全新 Windows 环境安装，走完整首启 → 三时段 → 次日复习
- **DoD**：spec §9「可打包出 .msi 安装包」达标

---

# Definition of Done（每个任务适用，5 层）

1. **代码层**：contracts-v1.md §12 禁止事项零触发
2. **测试层**：新增核心逻辑有单测 + ≥2 条异常路径；不靠 mock 一切造假绿
3. **集成层**：DI/入口层注入真依赖；实启 `npm run tauri dev` 并操作验证
4. **验证层**：`cargo clippy` + `npx oxlint` + `cargo test` + `npm test` 全绿
5. **Spec 层**：输出 ✅完整 / ⚠️部分+缺什么 / ❌未实现+原因 / 🔧偏离+理由 四态对齐报告

**Phase 出口额外门禁**：Phase 1/3/5 结束时必须实跑 + 直接查 DB / 听音频 / 真机操作，**不接受「测试全过」作为完成标志**。

---

# 风险与回退

| 风险 | 影响 | 缓解 | 回退 |
|---|---|---|---|
| 公开词库质量差（音标/释义错误） | 直接教错单词，最高危 | T15 抽样 30 词人工核对；多源交叉验证 | 缩到 500 词高频切片，人工精校后先上线 |
| AI 例句语法错误或语境跑偏 | 学习质量受损 | T17 抽样 50 词审阅 + 自动语法检查 | 例句降级为「仅 example_1」，或退回中性例句 |
| Windows API 在真机行为与文档不符 | F1 弹窗不可用 | T24 尽早真机验证，不放到最后 | 降级为固定间隔弹窗 + 手动「稍后」，标注 MOCKS.md |
| 弹窗抢焦点打断游戏 | 直接触发用户逆反，产品性失败 | T26 真机反复验证 `focus:false` | 改为托盘闪烁提醒，不弹窗 |
| Edge-TTS 需联网而目标机常离线 | Lv.3 听音题不可用 | T19 预生成全部音频随包分发 | SAPI 兜底；或 Lv.3 题型暂时下线 |
| ts-fsrs 默认参数不适配 15 岁初学者 | 复习间隔过长/过短 | review_logs 记全 before/after，装机一周后可回溯调参 | 手动收紧 `requestRetention` |

---

# 范围外（明确不做）

- spec M2/M3 的 F8–F13（主界面完整版、家园建造、魔王讨伐、赛季赛道、抽卡、周报邮件）→ V1.1+
- P2 全部（看板娘、皮肤系统、CSV 导入、留存率报表）
- 手机端、云同步、多用户、防卸载

---

# 已确认事项（2026-08-05）

| # | 问题 | 结论 |
|---|---|---|
| 1 | 目标机联网 | **常联网**。但 T19 仍采用「全量预生成随包分发」——spec F4 要求点击后 300ms 内播放，实时合成无法保证；联网仅用于增量补全与更新 |
| 2 | 词库教材版本 | **人教版 + 外研版融合**（并集去重，标注来源版本） |
| 3 | 每日新词 6 个 | **确认**，作为上限值；运行时受强化池自适应调制（contracts §4.1） |
| 4 | 今日暂停月配额 2 次 | **确认**，语义定为「冻结」（contracts §7.1） |
| 5 | 学习周期 | 从「学年内 5100 词」改为 **2 年内覆盖并基本掌握**（决议 S2） |
| 6 | 摸底策略 | 三阶段渐进分级（contracts §9），识别并跳过已掌握的初中词 |
| 7 | MVP 留存钩子 | **极简抽卡提前进 MVP**（决议 S9，新增 T31） |
| 8 | 周报邮件（F13） | 属 V1.1，SMTP 配置届时再定 |

> spec 业务逻辑的 12 项问题与决议见 [../spec-review-2026-08-05.md](../spec-review-2026-08-05.md)。
