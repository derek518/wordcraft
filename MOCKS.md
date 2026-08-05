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
| M2 | `src-tauri/src/tts.rs` | `play_word_audio` 创建目录后直接 `Ok(())`，从不发声 | **T19** | 待清除 |
| M3 | `src-tauri/src/fsrs_engine.rs` | 未使用任何 FSRS 库；`generate_options` 返回硬编码 `"选项A/B/C/D"` | **T11**（整文件删除） | 待清除 |
| M4 | `src-tauri/src/db/legacy.rs` | JSON 文件存储替代 SQLite；手写日期函数（`86464` typo，85% 日期算错）；`add_days` 忽略天数参数 | **T10** | ⏳ 部分 — 见下 |
| M5 | `src/components/WordTrainer.tsx:86` | 50 个硬编码中文释义数组充当干扰项 | **T20** | 待清除 |
| M6 | `src/components/WordTrainer.tsx:64` | `catch` 中静默 fallback 到本地 `words.ts` 假数据，掩盖后端故障 | **T21** | 待清除 |
| M7 | `src/components/AdventureMap.tsx:27` | `completedToday` 是永远为空的 `useState`，传送门完成状态是假的 | **T21** | 待清除 |
| M8 | `src/data/words.ts` | 52 词硬编码词库，且 100% 以字母 a 开头（字典序前 52 个，非按词频） | **T18** | 待清除 |

#### M4 进展（T06 完成时）

- ✅ SQLite schema 已建立并接入启动流程（`db::init` 在 `main.rs` setup 中执行，失败即终止启动）
- ✅ 遗留 JSON 已归档（带标记防重复归档），其数据不迁移——时间戳由缺陷日期函数生成，不可信
- ⏳ **7 个 command 仍走 `db::legacy`**（`main.rs` 中以 `// TODO(T07)` 标记），JSON 存储与手写日期函数随之存活
- ⏳ `db::legacy::init_database` 已删除（settings 初始化改由 migration 001 负责）
- ✅ **T07 完成**：Repository 层（6 模块 76 测试）+ `clock` 时间模块就位，新代码零手写日期运算
- ⏳ 清除时机修正为 **T10**——legacy 只有在全部 command 切换到 repo 之后才能删，而 command 实现分布在 T08/T09/T10

**M4 完全清除的判据**：`src-tauri/src/db/legacy.rs` 文件不存在，且 `main.rs` 的 `generate_handler!` 中无 `db::legacy::` 路径。

### 🟡 计划内 stub（尚未引入，T23 建立）

| # | 位置 | 说明 | 约束 |
|---|---|---|---|
| S1 | `src-tauri/src/platform/stub.rs` | 非 Windows 平台的 `PlatformIntegration` 实现 | **永久保留**（开发机需要），但必须返回 `BusyState::Unknown` 并记 warn 日志，**禁止返回 `Normal` 伪装成正常**。真实能力仅在 Windows 实现中 |

> S1 是本清单中唯一允许长期存在的 stub。它的正当性在于：开发机为 macOS，而 `SHQueryUserNotificationState` 是 Windows 专有 API。
> 它之所以安全，是因为返回 `Unknown` 强制调用方显式处理——**能力缺失无法伪装成一切正常**。这正是审计 M6 那类 silent fallback 的反面。

## 清除进度

| Phase | 应清除 | 已清除 |
|---|---|---|
| Phase 1（T06/T07） | M4 | — |
| Phase 2（T11） | M3 | — |
| Phase 3（T18/T19） | M2, M8 | — |
| Phase 4（T20/T21） | M5, M6, M7 | — |
| Phase 5（T25） | M1 | — |

**Phase 6 结束时本表「存量债务」区必须清空。** 若届时仍有残留，不得进入 spec M1 MVP 验收。

## CI 检查（Phase 0 后启用）

```bash
# 业务代码中的 TODO 必须全部登记在本文件
grep -rn "TODO(T" wordcraft/src wordcraft/src-tauri/src | sort > .mocks-current
# 与本文件比对，出现未登记项则失败
```
