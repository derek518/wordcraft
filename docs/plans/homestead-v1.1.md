# F9 家园建造 · 契约

> spec §4.2 F9「长期留存核心」。欢迎页已向用户承诺「收集的水晶可以用来建造家园」，
> 而当前无任何入口——这是一张已经开出去的空头支票。
>
> 本文只定契约（schema / command / 规则 / 断言意图），不含组件代码。

---

## 1. 两处偏离 spec，及理由

### 1.1 方块按「实际作答」而非「有学习记录」发放

spec 原文是「每收集 1 颗水晶 = 1 个普通方块」。实测数据推翻了这条的字面实现：

```
word_states 总计   1589
  其中 reps > 0      83   ← 真正答过的
  摸底预分级        1506   ← 估算「你可能认识」
```

摸底判定「你大概会这个词」不等于「你收集了这颗水晶」。按 `word_states` 存在与否
发放，用户做完摸底立刻凭空得到 1500 多个方块，建造这件事在第一天就失去意义。

**发放基准取 `reps > 0`。** 当前发放 83 块，随实际练习增长。

### 1.2 库存可超出网格容量

20×20 = 400 格，而词库全部练完会产出 3657 块。spec 写「1:1」时假设词汇量线性
缓慢增长，没有计入摸底与长期积累。

**不改比例，改语义**：库存是「拥有的资源」，网格是「展示的空间」。放置消耗库存，
移除退回库存，用户自己决定摆什么。过剩的方块不是缺陷——它是「我练了这么多」的
量化证明。

---

## 2. Schema（migration 006）

```sql
-- 方块库存。按类型聚合，不逐块记录
CREATE TABLE block_inventory (
  block_type TEXT PRIMARY KEY,
  owned      INTEGER NOT NULL DEFAULT 0,
  placed     INTEGER NOT NULL DEFAULT 0,

  CHECK (block_type IN ('normal', 'rare', 'limited')),
  CHECK (owned >= 0),
  CHECK (placed >= 0 AND placed <= owned)
);

-- 家园网格。只存已放置的格子，空格不占行
CREATE TABLE homestead_grid (
  x          INTEGER NOT NULL,
  y          INTEGER NOT NULL,
  block_type TEXT NOT NULL,
  placed_at  TEXT NOT NULL,

  PRIMARY KEY (x, y),
  FOREIGN KEY (block_type) REFERENCES block_inventory(block_type),
  CHECK (x BETWEEN 0 AND 19),
  CHECK (y BETWEEN 0 AND 19)
);

-- 发放账本。source + source_key 唯一约束是幂等的全部保障：
-- 发放会在每次启动、每次作答后触发，没有这个约束就会重复发
CREATE TABLE block_grants (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  source     TEXT NOT NULL,
  source_key TEXT NOT NULL,
  block_type TEXT NOT NULL,
  amount     INTEGER NOT NULL,
  granted_at TEXT NOT NULL,

  UNIQUE (source, source_key),
  CHECK (source IN ('mastery', 'streak', 'milestone')),
  CHECK (amount > 0)
);

INSERT OR IGNORE INTO block_inventory (block_type) VALUES
  ('normal'), ('rare'), ('limited');
```

`source_key` 的取值约定：

| source | source_key | 触发 |
|---|---|---|
| `mastery` | `word_id` | 该词首次 `reps > 0` |
| `streak` | 达成日期 `YYYY-MM-DD` | 连续打卡每满 7 天 |
| `milestone` | 里程碑词数（`200` / `500` / …） | 累计作答词数跨过阈值 |

---

## 3. Command 签名

```rust
/// 家园全貌：网格 + 库存。一次取全，避免前端拼装两次请求
get_homestead() -> Result<HomesteadState, String>

/// 放置。库存不足或格子已占用返回 Err，禁止静默失败
place_block(x: i64, y: i64, block_type: String) -> Result<HomesteadState, String>

/// 移除，方块退回库存
remove_block(x: i64, y: i64) -> Result<HomesteadState, String>

/// 补发所有未发放的方块。启动时与每次会话结束后调用，幂等
grant_pending_blocks() -> Result<GrantOutcome, String>
```

```rust
pub struct HomesteadState {
    pub grid: Vec<PlacedBlock>,          // 仅已放置的格子
    pub inventory: Vec<BlockStock>,      // 三种类型各自的 owned / placed
    pub grid_size: i64,                  // 20，前端不硬编码
}

pub struct PlacedBlock { pub x: i64, pub y: i64, pub block_type: String }
pub struct BlockStock { pub block_type: String, pub owned: i64, pub available: i64 }

pub struct GrantOutcome {
    pub granted: Vec<(String, i64)>,     // (block_type, 本次新增)
    pub total_available: i64,
}
```

**返回整个 `HomesteadState` 而非 `()`**：放置后前端要同步更新网格与库存两处，
让后端回一份权威快照，比前端各自推算再对账可靠。

---

## 4. 发放规则

```
mastery   每个词首次 reps > 0        → normal ×1
streak    best_streak 每满 7 的倍数   → limited ×1
milestone 累计作答词数跨过阈值        → rare ×1
          阈值 [200, 500, 1000, 2000, 3657]
```

`rare` 在 spec 里的来源是「击败魔王」（F10 未做）。里程碑暂代其来源，F10 落地后
两者并存——发放走同一张账本，`source` 区分即可，无需改 schema。

**追溯发放**：首次执行时，对所有 `reps > 0` 的词补发。账本的唯一约束保证
后续启动不会重复。

---

## 5. 测试断言意图

### Rust（in-memory sqlite）

| 断言 | 为什么这条重要 |
|---|---|
| 同一个词重复调用发放，`normal` 只 +1 | 幂等是全部正确性的基础，发放会被反复触发 |
| 摸底预分级的词（`reps = 0`）不发方块 | 否则做完摸底凭空得 1500 块，建造失去意义 |
| 追溯发放后再答新词，只增量发 1 块 | 追溯与增量共用一条路径，不能互相干扰 |
| 库存不足时放置返回 Err | 静默失败会让用户以为放上了，刷新后消失 |
| 已占用的格子再放置返回 Err | |
| 移除后 `placed` 减 1，`available` 回升 | 退回逻辑写反会让方块凭空消失 |
| 坐标越界（-1 / 20）返回 Err | CHECK 约束能挡，但错误信息无法诊断 |
| `placed` 永不超过 `owned` | 用 CHECK 保证，测试验证约束真的生效 |
| streak 达 14 天发 2 块 limited，不是 1 块 | 每满 7 的倍数各发一次，跨越多个阈值时不能漏 |
| 里程碑跨越多个阈值时逐个发放 | 摸底后可能一次跨过 200 和 500 |

### 前端（vitest）

| 断言 | |
|---|---|
| 网格渲染 20×20，空格可点 | `grid_size` 来自后端，不硬编码 |
| 库存为 0 的类型不可选 | |
| 放置失败时显示后端错误原文 | 不吞错、不静默回滚 |

---

## 6. 文件

**新建**

```
wordcraft/src-tauri/src/db/migrations/006_homestead.sql
wordcraft/src-tauri/src/db/repo/homestead.rs      库存与网格读写
wordcraft/src-tauri/src/homestead/mod.rs          command 层
wordcraft/src-tauri/src/homestead/grants.rs       发放规则（纯逻辑，可穷举测试）
wordcraft/src/components/Homestead.tsx            家园页
scripts/cards/generate_blocks.py                  方块素材扩充
```

**修改**

```
wordcraft/src-tauri/src/db/migrations.rs          注册 006
wordcraft/src-tauri/src/main.rs                   注册 command
wordcraft/src/data/api.ts                         接口封装
wordcraft/src/App.tsx                             视图路由
wordcraft/src/components/AdventureMap.tsx         家园入口
wordcraft/src-tauri/src/commands/session.rs       会话结束后触发发放
```

---

## 7. 任务拆分

| # | 任务 | 依赖 | 产出 |
|---|---|---|---|
| H1 | migration 006 + repo 层 | — | schema 落地，repo 单测通过 |
| H2 | 发放规则（纯逻辑 + 幂等） | H1 | 追溯 83 块，重复执行不增 |
| H3 | 网格 command（放置 / 移除 / 查询） | H1 | 边界与库存校验完备 |
| H4 | 方块素材 | — | 三类方块 + 若干装饰变体 |
| H5 | 前端家园页 | H2 H3 H4 | 可放置、可移除、库存实时 |
| H6 | 蓝图系统 | H5 | 小屋 / 城堡 / 村庄 / 城市 |

H1–H3 是后端，可连续做；H4 独立；H5 依赖前三者。**H6 蓝图放最后**——它是
锦上添花，前五项完成时家园已经可用。

---

## 8. DoD

沿用项目的五层定义。本功能额外要求：

- 实跑启动后 `block_inventory.owned` 为 **83**（当前 `reps > 0` 的词数），
  不是 1589
- 重启应用三次，`owned` 保持不变（幂等）
- 在 UI 中放置、移除、再放置，`sqlite3` 直接查 `homestead_grid` 与
  `block_inventory` 两表数据一致
- 关闭应用重开，已放置的方块仍在原位

---

## 9. 未决

- **蓝图的呈现方式**：是「按蓝图自动摆放」还是「显示轮廓引导手动摆」？
  前者省事但失去建造感，后者更有参与感但要做轮廓叠加层。留到 H6 再定。
- **网格是否可扩**：spec 定 20×20。若后期方块严重过剩，可考虑按里程碑解锁
  更大网格，但那会改动 schema 的 CHECK 约束，需要新 migration。
