# 素材来源清单

> spec F12 验收项：仓库内附素材来源与许可证清单，全部可追溯。
> 素材约束见 contracts §10.3：仅公有领域（Wikimedia Commons PD 标记）或原创生成，
> **禁止任何商业游戏的角色名、立绘、贴图**。

## 卡池 B · 原创像素生物（16 张）

| 项 | 内容 |
|---|---|
| 位置 | `wordcraft/public/cards/creatures/` |
| 生成脚本 | `scripts/cards/generate_creatures.py` |
| 许可证 | CC0（原创，无第三方素材） |
| 清单 | `scripts/cards/creatures.json` |

程序化生成：形态由 (形态, 元素, 稀有度) 三个参数决定，无手绘资源、无外部依赖。
重跑脚本即可复现全部素材。

**参数组合必须唯一**——图案完全由 (形态, 元素) 决定，组合撞车会产出两张完全
相同的卡。脚本内置断言拦截，此前「石背龟」与「沙丘鼠」正是这样撞在一起的。

## 卡池 A · 公有领域名画（待补）

contracts §10.2 规定卡池 A 为世界名画像素化（星月夜、神奈川冲浪里等）。
**尚未纳入**：需要从 Wikimedia Commons 下载 PD 原图并像素化处理。

补入时必须：

- 逐张核对 Wikimedia 的 PD 标记（作者逝世满 70 年 / 明确 PD 声明）
- 原图不入库，仅提交像素化处理后的成品
- 每张在本文件登记：作品名、作者、年代、Wikimedia 页面 URL、许可证标记
- 处理脚本置于 `scripts/cards/`

## UI 与游戏素材

| 项 | 内容 |
|---|---|
| 位置 | `wordcraft/public/assets/` |
| 生成脚本 | `public/assets/generate_assets.py` |
| 许可证 | CC0（原创像素图） |
| 说明 | 见 `public/assets/README.md` |

## 词库

| 项 | 内容 |
|---|---|
| 来源 | ECDICT 考纲词汇（gk ∪ zk） |
| 例句 | deepseek-v4-flash 生成，经 contracts §8 校验 |
| 构建 | `scripts/wordlist/` |
| 详情 | `scripts/wordlist/SOURCES.md` |
