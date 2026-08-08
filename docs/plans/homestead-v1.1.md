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

> migration 008 重建 `block_grants`，把 `'boss'` 加进 source 白名单——魔王掉落走同一张账本。

### 2.1 居民（migration 009）

```sql
CREATE TABLE homestead_residents (
  slot        INTEGER PRIMARY KEY,
  card_id     INTEGER NOT NULL REFERENCES cards(id),
  moved_in_at TEXT    NOT NULL,

  -- 同一只生物不能同时住两个位置。没有这条约束，用户会把唯一一张
  -- 稀有卡填满所有位置，收集的意义随之消失
  UNIQUE (card_id)
);
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

### 3.1 居民（见 §10.3）

```rust
/// 入住位、已入住、可入住的候选、以及住户要转述的数字，一次取全
get_residents() -> Result<ResidentsState, String>

/// 位置已有住户时先请出去——用户点的是「换成这只」
move_in_resident(slot: i64, card_id: i64) -> Result<ResidentsState, String>

move_out_resident(slot: i64) -> Result<ResidentsState, String>
```

```rust
pub struct ResidentsState {
    pub slots: i64,                  // 已解锁，由建成的蓝图数决定
    pub max_slots: i64,              // 6
    pub completed: Vec<String>,      // 已建成的蓝图 id
    pub residents: Vec<Resident>,
    pub candidates: Vec<Resident>,   // 已收集但未入住的生物
    pub digest: Digest,
}

pub struct Resident {
    pub slot: i64,                   // 候选时为 -1
    pub card_id: i64,
    pub name: String,
    pub image_path: String,
    pub rarity: i64,
}

pub struct Digest {
    pub due_count: i64,
    pub available_blocks: i64,
    pub streak: i64,
    pub words_to_milestone: i64,     // 0 表示没有下一档，不是「差 0 个」
}
```

**槽位上限在服务端复核**：前端拿到的 `slots` 可能已经过期（另一处刚拆了方块），
以入住那一刻的实际状态为准。

---

## 4. 发放规则

```
mastery   每个词首次 reps > 0        → normal ×1
streak    best_streak 每满 7 的倍数   → limited ×1
milestone 累计作答词数跨过阈值        → rare ×1
          阈值 [200, 500, 1000, 2000, 3657]
```

`rare` 在 spec 里的来源是「击败魔王」。里程碑暂代其来源；F10 已落地（`src-tauri/src/boss.rs`），
两者并存——发放走同一张账本，`source` 区分（`milestone` / `boss`），未改 schema。
同一个词只掉落一次，否则可以故意答错让它变回魔王反复刷。

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

- **网格是否可扩**：spec 定 20×20。若后期方块严重过剩，可考虑按里程碑解锁
  更大网格，但那会改动 schema 的 CHECK 约束，需要新 migration。

---

## 10. 可玩性重做（2026-08-08）

首版落地后复盘，发现三个结构性问题，均已修正。

### 10.1 四张蓝图互相排斥

首版是四张独立字符画，包含关系靠人工维护。实测 `hut → castle` 只有
**3/34** 格能留下——名义上的「小屋→城堡→村庄→城市」成长链，实现上是四次
推倒重来。用户建完小屋点城堡，会发现自己的小屋正挡在路中间。

改为**双层字符画**：一层写方块类型，一层写它在第几阶段出现，第 N 阶段 =
所有 `stage <= N` 的格子。包含关系成了结构性的，改图案时想破坏都破坏不掉。

画面概念是天际线：小屋在最高处，聚落从它脚下长出来，第一间屋子永远看得见。

| 阶段 | 累计块数 | 普通 | 稀有 | 限定 |
|---|---|---|---|---|
| 小屋 | 24 | 24 | 0 | 0 |
| 城堡 | 78 | 75 | 2 | 1 |
| 村庄 | 130 | 125 | 3 | 2 |
| 城市 | 195 | 187 | 5 | 3 |

### 10.2 蓝图需求超出方块供给

首版小屋需要 **15 块稀有方块**，而里程碑一共只发 5 块——作为「第一个目标」
实际要几个月才够。四条描述里还有三条与图案对不上（写 28 实为 34）。
七个测试没有一条检查供需。

现在：稀有需求封顶 5（= 里程碑总数），限定封顶 3（= 21 天连续），
且**小屋不依赖稀有与限定**——第一个目标必须只靠「继续答题」就能达成，
卡在里程碑或连续打卡上会让第一次成就感推迟到用户已经放弃之后。
描述不再写块数，数量只从图案算。四条断言守着这些约束。

### 10.3 完成蓝图没有任何结果

首版 `matched === cells.length` 时什么都不发生。这是家园最大的空洞：
建造没有结果，也就没有回来的理由。

现在完成蓝图解锁**入住位**，收集到的生物可以住进来（migration 009）。
抽卡第一次有了用处，家园第一次有了活物。

- 入住位：小屋 +1、城堡 +1、村庄 +2、城市 +2，累计 6
- 位置数（6）刻意少于生物卡池（16）——「让谁住进来」才是个选择
- 画作不能入住：它们是挂在墙上的，不是活物
- 同一只生物不能占两个位置（`UNIQUE (card_id)`），否则一张稀有卡能填满全部
- 拆掉方块使蓝图失效时，位置收回、居民自动搬离，且能重新入住

住户还会转述几个真实数字（到期词数、可用方块、连续天数、距下个里程碑），
让家园顺带成为一块软性的信息面板。数字在后端算，措辞留给前端。

### 10.4 建造摩擦

逐格点击建小屋要点 24 次，建城市要 195 次。网格改为**按住连续涂抹**，
方向由起点决定（起点是空格就一路放，是方块就一路拆）——来回划过同一格
反复放了又拆，是最容易误操作的地方。
