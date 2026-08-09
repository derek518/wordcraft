# 素材来源清单

> spec F12 验收项：仓库内附素材来源与许可证清单，全部可追溯。
> 素材约束见 contracts §10.3：仅公有领域（Wikimedia Commons PD 标记）或原创生成，
> **禁止任何商业游戏的角色名、立绘、贴图**。

## 卡池 v2 · AI 辅助生成像素卡（42 张）

> migration 010 起启用。取代下方卡池 A / B——那两批的素材文件仍在仓库中，
> 但已无任何运行时引用。

| 项 | 内容 |
|---|---|
| 位置 | `wordcraft/public/assets/cards/{common,rare,legend}/` |
| 构成 | 普通 24（碎片 6 · 生物 12 · 器物 6）· 稀有 12（守护者 6 · 神器 6）· 传说 6 |
| 许可证 | **CC0** — 原创构思，无第三方素材、无临摹对象 |
| 提示词 | `docs/card-art-prompts.md`（唯一真相来源） |
| 平铺提示词 | `scripts/cards/card_prompts.tsv`，由 `scripts/cards/build_prompts.py` 从文档派生 |
| 压制脚本 | `scripts/cards/conform.py` |

### 生成方式必须写清楚

这批素材由**图像生成模型产出、经脚本压制**，`cards.source` 列统一记
`原创生成 · AI 辅助 · CC0`。

原图左下角带生成方烙的「AI生成」灰度水印，压制时清除——**那是画面上的一行字，
不是出处**。抹掉字不等于抹掉出处，故在此明确记录：42 张卡面均为 AI 辅助生成。

### 为什么仍算 CC0 且不违反素材约束

- 提示词中不出现任何游戏、动画、IP 的名称，一个都没有
- 负面提示词固定带 `existing franchise character, recognisable game mascot`
- 逐张目视核对过，无可辨认的既有角色形象
- 构思（42 个主体、剪影分配、稀有度分档）为本项目原创，见提示词文档 §6–§9

### 复现

```bash
python3 scripts/cards/build_prompts.py > scripts/cards/card_prompts.tsv
# 按 TSV 生成 1024×1024 原图到暂存目录
python3 scripts/cards/conform.py <暂存目录> -o wordcraft/public/assets/cards/<tier>
```

与程序化生成的卡池 B 不同，这批**不能逐像素复现**——图像模型有随机性。
提示词与压制参数可复现，具体画面不可。这是采用 AI 生成的固有代价，记录在此。

## 卡池 B · 原创像素生物（16 张）

> 已由卡池 v2 取代（migration 010），素材保留但无运行时引用。

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

## 卡池 A · 公有领域名画（8 张）

> 已由卡池 v2 取代（migration 010），素材保留但无运行时引用。

| 项 | 内容 |
|---|---|
| 位置 | `wordcraft/public/cards/paintings/` |
| 下载与处理脚本 | `scripts/cards/fetch_paintings.py` |
| 清单 | `scripts/cards/paintings.json` |

**许可核验由代码强制，不靠人工判断。** 脚本经 Wikimedia API 查询每张画的
`LicenseShortName`，不在 PD 白名单内的直接拒绝下载。白名单逐项列举而非模糊
匹配——`grep -i public` 会把 "Public domain in the US only" 也放进来。

原图不入库，仅提交像素化成品（48×48 降采样 + 32 色量化 + NEAREST 放大）。
48 而非 16：名画的辨识依赖构图，压到 16 格后星月夜和睡莲会变成两坨相似色块。

| 作品 | 稀有度 | 许可 | Wikimedia 页面 |
|---|---|---|---|
| 星月夜 | 稀有度 3 | Public domain | https://commons.wikimedia.org/wiki/File:Van_Gogh_-_Starry_Night_-_Google_Art_Project.jpg |
| 神奈川冲浪里 | 稀有度 3 | Public domain | https://commons.wikimedia.org/wiki/File:Great_Wave_off_Kanagawa2.jpg |
| 呐喊 | 稀有度 3 | Public domain | https://commons.wikimedia.org/wiki/File:Edvard_Munch,_1893,_The_Scream,_oil,_tempera_and_pastel_on_cardboard,_91_x_73_cm,_National_Gallery_of_Norway.jpg |
| 戴珍珠耳环的少女 | 稀有度 2 | Public domain | https://commons.wikimedia.org/wiki/File:1665_Girl_with_a_Pearl_Earring.jpg |
| 向日葵 | 稀有度 2 | Public domain | https://commons.wikimedia.org/wiki/File:Vincent_Willem_van_Gogh_127.jpg |
| 大碗岛的星期天下午 | 稀有度 2 | Public domain | https://commons.wikimedia.org/wiki/File:A_Sunday_on_La_Grande_Jatte,_Georges_Seurat,_1884.jpg |
| 睡莲 | 稀有度 1 | Public domain | https://commons.wikimedia.org/wiki/File:Claude_Monet_-_Water_Lilies_-_1906,_Ryerson.jpg |
| 拾穗者 | 稀有度 1 | Public domain | https://commons.wikimedia.org/wiki/File:Jean-Fran%C3%A7ois_Millet_(II)_013.jpg |

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
