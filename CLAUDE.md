# WordCraft 词匠

游戏化单词记忆桌面应用（Tauri 2 + React）。常驻 Windows 托盘，每天早/中/晚三时段自动弹出 ≤120 秒的单词训练，用 FSRS 间隔重复自动识别掌握程度。

目标用户为高中生，产品设计针对 ADHD 特征优化。**目标平台 Windows 10/11**；macOS 用于开发，Windows 专有能力由 stub 替代。

## 仓库结构

```
wordcraft-spec.md / wordcraft-spec-v1.0.md   原始产品规格
docs/
├── audit-2026-08-05.md                      代码审计报告
├── spec-review-2026-08-05.md                spec 业务逻辑审查与决议
└── plans/
    ├── contracts-v1.md                      ★ 契约：schema / command 签名 / 状态机 / 算法参数
    └── rollout-v1.md                        实施计划 Phase 0–6，31 个任务
wordcraft/                                   应用代码
├── src/                                     前端（components / core / data）
├── src-tauri/src/                           后端（db / platform / scheduler / tts）
└── public/assets/                           像素素材（代码生成，见 assets/README.md）
MOCKS.md                                     mock/stub 库存清单
```

## 最重要的约定

**`docs/plans/contracts-v1.md` 是实施的唯一准绳。** 它与原始 spec 有意偏离之处，均记录在 `docs/spec-review-2026-08-05.md`（12 项决议）。遇到 spec 与契约冲突，以契约为准；要改契约，先改文档再改代码，同一 commit 提交。

## 常用命令

```bash
npm run tauri dev        # 完整应用
npm run dev              # 仅前端
npm run build            # 类型检查 + 构建
npm run lint             # oxlint（要求零警告）
npm test                 # 前端单测

cd src-tauri
cargo check              # 类型检查
cargo clippy             # lint（要求零警告）
cargo test               # 后端单测
```

## 环境约束

- **网络**：crates.io 的 sparse index 在此环境下延迟高，cargo 命令需要
  `CARGO_NET_RETRY=10 CARGO_HTTP_TIMEOUT=120`，否则会瞬时超时。
- **macOS 上无法用 computer-use 操作 `tauri dev` 启动的窗口**：dev 模式跑的是
  `target/debug/wordcraft` 裸二进制，没有 `.app` bundle，不在系统应用列表中。
  需要真机 UI 验证时先 `npm run tauri build -- --debug --bundles app`，
  再 `open src-tauri/target/debug/bundle/macos/WordCraft.app`。
- **`rusqlite` 锁定在 0.37.x，不要升级**。0.40+ 依赖 `libsqlite3-sys` 0.38+，
  其 build.rs 使用了在 Rust 1.93 仍为 unstable 的 `cfg_select!`，编译直接失败。
  升级前需先确认工具链已稳定支持该特性。

## 架构要点

| 决策 | 内容 |
|---|---|
| ADR-1 | SQLite（`rusqlite`，非 `tauri-plugin-sql`）——数据库仅 Rust 可访问，前端无法绕过 command 契约直接写库 |
| ADR-2 | **FSRS 在前端**（`ts-fsrs`）；Rust 只做持久化与按 `due_at` 查询排队 |
| ADR-3 | 平台抽象 trait 隔离 Windows API；非 Windows 的 stub **必须返回 `Unknown` 并记 warn**，禁止伪装成正常 |
| ADR-4 | 全部日期时间走 `chrono`，**禁止手写日历运算** |
| ADR-5 | 存储时间戳一律 UTC ISO8601；"今天"的归属按本地时区 |
| ADR-6 | `fsrs_state`（算法拥有）与 `app_state`（产品状态机）分为两列，语义不同 |

## 禁止事项

- ❌ 业务代码出现 mock / 硬编码释义数组 / `"选项A"` / 硬编码时间值
- ❌ `catch` 后静默 fallback 到本地假数据（后端失败必须显示错误态）
- ❌ 手写日期/日历运算
- ❌ `// simplified for MVP` / `// In production, would...` / `// In a real app...` 类话术
- ❌ command 返回 `Ok(())` 但实际什么都没做
- ✅ 唯一例外：`// TODO(T<NN>): ...` 形式的占位符，且必须同步登记进 `MOCKS.md`

素材必须原创或公有领域（CC0）。**禁止任何商业游戏的角色名、立绘、贴图、音乐**；风格致敬不受此限。

## 完成定义（5 层，缺一不可）

1. **代码层** — 上述禁止事项零触发
2. **测试层** — 核心逻辑有单测 + ≥2 条异常路径；不靠 mock 一切造假绿
3. **集成层** — 入口层注入真依赖；实启 `npm run tauri dev` 操作验证
4. **验证层** — `cargo clippy` + `oxlint` + `cargo test` + `npm test` 全绿
5. **Spec 层** — 输出 ✅完整 / ⚠️部分+缺什么 / ❌未实现+原因 / 🔧偏离+理由 四态报告

**Phase 1/3/5/6 出口额外门禁**：必须实跑并直接查 DB、听音频、真机操作。**不接受「测试全过」作为完成标志**——本项目审计已证明这类假绿的代价。

## 工作方式

- 一次一个任务，做完一个 commit 一个；禁止 batch commit 无关变更
- 任务编号沿用 `docs/plans/rollout-v1.md` 的 T01–T31
- commit message 用英文，Conventional Commits 格式
