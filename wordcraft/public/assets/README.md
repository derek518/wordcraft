# WordCraft 素材清单

> 所有素材均为代码生成的像素风格 PNG，无版权风险，风格统一。
> 生成脚本: `public/assets/generate_assets.py`（可重新运行生成/修改）

---

## 目录结构

```
public/assets/
├── ui/                    # UI 元素
│   ├── app_icon_32.png    # 系统托盘图标 (32x32)
│   ├── app_icon_256.png   # 应用图标 (256x256)
│   ├── app_icon_512.png   # 高分辨率图标 (512x512)
│   ├── chest_small.png    # 小宝箱 (128x128)
│   ├── chest_large.png    # 大宝箱 (128x128)
│   ├── boss.png           # 遗忘魔王 (192x192)
│   └── zone_*.png         # 6个区域地图预览 (128x128)
│
├── portals/               # 传送门
│   ├── portal_morning.png # 晨曦之门 (256x256)
│   ├── portal_noon.png    # 烈日之门 (256x256)
│   └── portal_evening.png # 星夜之门 (256x256)
│
├── crystals/              # 元素水晶 (6元素 × 3状态 = 18张)
│   ├── crystal_grass_bright.png   # 草-明亮
│   ├── crystal_grass_faint.png    # 草-微光
│   ├── crystal_grass_dim.png      # 草-灰暗
│   ├── crystal_water_bright.png   # 水-明亮
│   ├── crystal_water_faint.png    # 水-微光
│   ├── crystal_water_dim.png      # 水-灰暗
│   ├── crystal_fire_bright.png    # 火-明亮
│   ├── crystal_fire_faint.png     # 火-微光
│   ├── crystal_fire_dim.png       # 火-灰暗
│   ├── crystal_thunder_bright.png # 雷-明亮
│   ├── crystal_thunder_faint.png  # 雷-微光
│   ├── crystal_thunder_dim.png    # 雷-灰暗
│   ├── crystal_ice_bright.png     # 冰-明亮
│   ├── crystal_ice_faint.png      # 冰-微光
│   ├── crystal_ice_dim.png        # 冰-灰暗
│   ├── crystal_rock_bright.png    # 岩-明亮
│   ├── crystal_rock_faint.png     # 岩-微光
│   └── crystal_rock_dim.png       # 岩-灰暗
│
├── blocks/                # 像素方块
│   ├── block_normal.png   # 普通方块 (64x64)
│   ├── block_rare.png     # 稀有方块 (64x64)
│   └── block_special.png  # 限定方块 (64x64)
│
├── badges/                # 成就徽章 (7个)
│   ├── badge_sprout.png     # 初出茅庐 🌱
│   ├── badge_fire.png       # 七日之火 🔥
│   ├── badge_sword.png      # 魔王克星 ⚔️
│   ├── badge_builder.png    # 建造师 🏠
│   ├── badge_collector.png  # 水晶收藏家 💎
│   ├── badge_perfect.png    # 百发百中 🎯
│   └── badge_night.png      # 夜行者 🌙
│
└── effects/               # 粒子特效
    ├── star.png           # XP星星 (90x90)
    └── sparkle.png        # 闪光 (90x90)
```

---

## 素材规格总览

| 类别 | 数量 | 尺寸 | 背景 | 用途 |
|------|------|------|------|------|
| 应用图标 | 3 | 32/256/512 | 透明 | 托盘/桌面/安装包 |
| 传送门 | 3 | 256×256 | 透明 | 主界面3个时段入口 |
| 水晶 | 18 | 128×128 | 透明 | 单词状态视觉表示 |
| 方块 | 3 | 64×64 | 透明 | 家园建造系统 |
| 宝箱 | 2 | 128×128 | 透明 | 通关奖励动画 |
| 魔王 | 1 | 192×192 | 透明 | 薄弱词讨伐战 |
| 徽章 | 7 | 128×128 | 透明 | 成就系统 |
| 特效 | 2 | 90×90 | 透明 | XP飘字/闪光动画 |
| 区域 | 6 | 128×128 | 不透明 | 地图区域预览 |

---

## 配色方案

| 元素 | 亮色 | 暗色 | 用途 |
|------|------|------|------|
| 草元素 | #4ADE80 | #22C55E | 初中基础词 |
| 水元素 | #3B82F6 | #2563EB | 初中核心词 |
| 火元素 | #EF4444 | #DC2626 | 高中核心词 |
| 雷元素 | #A855F7 | #9333EA | 高中拓展词 |
| 冰元素 | #67E8F9 | #22D3EE | 高考高频词 |
| 岩元素 | #F59E0B | #D97706 | 美术专业词 |

---

## 前端使用方式

```tsx
// 在 React 组件中引用
<img src="/assets/crystals/crystal_fire_bright.png" width={32} />
<img src="/assets/portals/portal_morning.png" width={64} />
<img src="/assets/badges/badge_collector.png" width={48} />
```

---

## 如何修改/重新生成

```bash
cd public/assets
python3 generate_assets.py
```

修改 `generate_assets.py` 中的像素网格定义即可调整素材外观。所有素材都是代码生成的，无需设计软件。
