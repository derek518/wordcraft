# MOCKS.md — mock / stub 库存清单

> 规则（依 `integration-discipline.md` §2.2）：任何 mock / stub / fake 进入代码**必须**登记在此，并标注清除责任任务。
> 每个任务 sign-off 前必检本文件：本任务引入的 mock 是否登记？上一任务承诺清除的是否已清除？**未清除不得 sign-off。**
>
> 代码中的占位符统一用 `// TODO(T<NN>): <说明>` 形式，`T<NN>` 对应 `docs/plans/rollout-v1.md` 的任务编号。

## 当前库存

### 🔴 存量债务（Phase 0 前既有，审计 2026-08-05 发现）

这些不是本轮引入的 mock，是审计发现的既有假实现。全部为待清除项。

| # | 位置 | 问题 | 清除任务 | 状态 |
|---|---|---|---|---|
| M1 | `src-tauri/src/scheduler.rs` | `start_scheduler` 仅 `sleep(60)` 死循环；`get_next_session_time` 硬编码返回 `"09:00"`/60 分钟；`trigger_popup_now` 直接 `Ok(())` | **T25** | 待清除 |
| M2 | `src-tauri/src/tts.rs` | `play_word_audio` 创建目录后直接 `Ok(())`，从不发声 | T19 | ✅ **已清除** |
| M3 | `src-tauri/src/fsrs_engine.rs` | 未使用任何 FSRS 库；`generate_options` 返回硬编码 `"选项A/B/C/D"` | T10 | ✅ **已清除** |
| M4 | `src-tauri/src/db/legacy.rs` | JSON 文件存储替代 SQLite；手写日期函数（`86464` typo，85% 日期算错）；`add_days` 忽略天数参数 | T10 | ✅ **已清除** |
| M5 | `src/components/WordTrainer.tsx` | 50 个硬编码中文释义数组充当干扰项 | T10（原定 T20） | ✅ **已清除** |
| M6 | `src/components/WordTrainer.tsx` | `catch` 中静默 fallback 到本地 `words.ts` 假数据，掩盖后端故障 | T10（原定 T21） | ✅ **已清除** |
| M7 | `src/components/AdventureMap.tsx` | `completedToday` 是永远为空的 `useState`，传送门完成状态是假的 | T10（原定 T21） | ✅ **已清除** |
| M8 | `src/data/words.ts` | 52 词硬编码词库，且 100% 以字母 a 开头（字典序前 52 个，非按词频） | **T18** | 待清除 |

#### T10 清除说明

M3–M7 在 T10 一并清除，早于原计划。原因是 **Tauri 的 command 命名空间全局唯一**——
新旧同名 command（`get_setting`、`get_overall_stats` 等）无法共存，`#[tauri::command]`
宏即使不注册也会生成冲突符号。强行保留 legacy 会造成数据分裂（词在 SQLite、作答在
JSON），比一次切干净更糟，故 T10 与 T11 合并执行。

清除后的事实判据（均已验证）：

- `src-tauri/src/db/legacy.rs` 与 `src-tauri/src/fsrs_engine.rs` 文件不存在
- `main.rs` 的 `generate_handler!` 中无 `db::legacy::` 与 `fsrs_engine::` 路径
- 前端一律经 `src/data/api.ts` 访问后端，该层**不含任何 fallback**——错误向上抛出
- 干扰项来自 `get_distractor_pool`（同词性候选池），无硬编码释义
- 传送门完成状态来自 `get_today_sessions`

#### M2 清除说明（T19 第一阶段）

`play_word_audio` 的假实现已替换为系统 TTS 实时合成——macOS 走 `say`，
Windows 走 PowerShell + `System.Speech`，两个平台都是真实现而非 stub
（发音在开发机上同样需要能听见，否则无从验证）。

**T19 的预生成部分仍待完成**：Edge-TTS 批量产出 mp3 随包分发，是 spec F4
「300ms 内出声」的正解，实时合成只是降级路径。但预生成必须等真实词库就位
（T15–T18）——为 52 个占位词生成音频，词库一换就全部作废。

这不构成 mock 债务：当前路径真的会发声，不是「返回 Ok 但什么都没做」。

M8 是唯一残留的存量债务，随 T18 的真实词库导入清除。

### 🟡 计划内 stub（尚未引入，T23 建立）

| # | 位置 | 说明 | 约束 |
|---|---|---|---|
| S1 | `src-tauri/src/platform/stub.rs` | 非 Windows 平台的 `PlatformIntegration` 实现 | **永久保留**（开发机需要），但必须返回 `BusyState::Unknown` 并记 warn 日志，**禁止返回 `Normal` 伪装成正常**。真实能力仅在 Windows 实现中 |

> S1 是本清单中唯一允许长期存在的 stub。它的正当性在于：开发机为 macOS，而 `SHQueryUserNotificationState` 是 Windows 专有 API。
> 它之所以安全，是因为返回 `Unknown` 强制调用方显式处理——**能力缺失无法伪装成一切正常**。这正是审计 M6 那类 silent fallback 的反面。

## 清除进度

| Phase | 应清除 | 状态 |
|---|---|---|
| Phase 1–2（T06–T10） | M3, M4, M5, M6, M7 | ✅ 全部清除 |
| Phase 3（T19） | M2 | ✅ 已清除 |
| Phase 3（T18） | M8 | 待办 |
| Phase 5（T25） | M1 | 待办 |

**Phase 6 结束时本表「存量债务」区必须清空。** 若届时仍有残留，不得进入 spec M1 MVP 验收。

## CI 检查（Phase 0 后启用）

```bash
# 业务代码中的 TODO 必须全部登记在本文件
grep -rn "TODO(T" wordcraft/src wordcraft/src-tauri/src | sort > .mocks-current
# 与本文件比对，出现未登记项则失败
```
