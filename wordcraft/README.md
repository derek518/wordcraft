# WordCraft 词匠

游戏化单词记忆桌面应用。常驻 Windows 系统托盘，每天早/中/晚三个时段自动弹出极短的单词训练（每次 ≤120 秒），通过 FSRS 间隔重复算法自动识别掌握程度，以「词汇冒险」世界观包装。

目标用户为高中生，产品设计针对 ADHD 特征优化：降低启动成本、极小剂量、即时反馈、可预测的全貌。

## 技术栈

| 层 | 技术 |
|---|---|
| 后端 | Rust · Tauri 2 |
| 前端 | React 19 · TypeScript · Vite |
| 样式 | Tailwind CSS 4（`@theme` 色板，支持换肤） |
| 存储 | SQLite |
| 算法 | ts-fsrs（前端计算，Rust 持久化） |
| 发音 | Edge-TTS 预生成 + Windows SAPI 兜底 |

**目标平台为 Windows 10/11。** macOS 可用于开发，但全屏检测与开机自启依赖 Windows API，在其他平台由 stub 实现替代（会记 warn 日志，不会伪装成正常）。

## 常用命令

```bash
npm install              # 安装前端依赖
npm run dev              # 仅前端开发服务器
npm run tauri dev        # 完整应用（Rust + 前端）
npm run build            # 前端类型检查 + 构建
npm run tauri build      # 打包 .msi / .nsis 安装包
npm run lint             # oxlint
npm test                 # 前端单元测试

cd src-tauri
cargo check              # Rust 类型检查
cargo clippy             # Rust lint（要求零警告）
cargo test               # Rust 单元测试
```

## 目录结构

```
src/                     前端
├── components/          UI 组件
├── core/                纯逻辑（FSRS 适配、评级映射、状态机、干扰项）
├── data/                内置数据
└── index.css            Tailwind @theme 色板与动画

src-tauri/src/           后端
├── db/                  SQLite 迁移与 Repository 层
├── platform/            平台抽象（Windows 实现 + 其他平台 stub）
├── scheduler.rs         弹窗调度
└── tts.rs               发音缓存

public/assets/           像素素材（代码生成，见 assets/README.md）
docs/                    审计报告、spec 审查、实施计划与契约
```

## 文档

| 文档 | 内容 |
|---|---|
| [../docs/plans/contracts-v1.md](../docs/plans/contracts-v1.md) | **契约** — schema、command 签名、状态机、算法参数。实施以此为准 |
| [../docs/plans/rollout-v1.md](../docs/plans/rollout-v1.md) | 实施计划 — Phase 0–6，31 个任务 |
| [../docs/audit-2026-08-05.md](../docs/audit-2026-08-05.md) | 代码审计报告 |
| [../docs/spec-review-2026-08-05.md](../docs/spec-review-2026-08-05.md) | spec 业务逻辑审查与决议 |
| [../wordcraft-spec-v1.0.md](../wordcraft-spec-v1.0.md) | 原始产品规格说明书 |

契约与代码必须同步演进——改契约的 commit 应当同时包含对应的代码变更。

## 素材约束

所有角色、图标、音效必须为原创生成或公有领域（CC0）。**禁止使用任何商业游戏的角色名、立绘、贴图、音乐**；风格致敬（像素方块感、元素属性配色）不受此限。

`public/assets/` 下的 45 个素材均由 `public/assets/generate_assets.py` 代码生成，可随时重跑修改。
