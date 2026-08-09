# 卡牌美术提示词 · 卡池 v2（42 张）

给图像生成模型用的完整提示词集。**目标不是单张好看，是 42 张像同一套。**

上一版的失败很具体：24 张普通卡里绝大多数是「一个纯色圆形 + 中间一个小符号」，
换个颜色就是另一张卡。本文用三个机制防止它再发生——

1. **剪影分配表**（§6）：每张卡预先指定一个互不重复的外形。缩略图大小下能靠轮廓区分，才算 42 张卡
2. **元素色阶表**（§5）：六个元素各锁 5 阶色，取自应用现有 UI 配色，卡面与界面同源
3. **后处理脚本**（§2）：不管模型生成什么，统一压到同一像素网格与同一调色板

---

## 1. 交付规格

| 项 | 值 |
|---|---|
| 最终文件 | `wordcraft/public/assets/cards/{common,rare,legend}/<原文件名>.png` |
| 尺寸 | **256 × 256**，RGBA（= 64 × 4，整数倍）|
| 背景 | **全透明**。边框、名牌、稀有度装饰由 `generate_cards.py` 合成，不要画进图里 |
| 逻辑像素 | 64 × 64（每个美术像素 = 4×4 设备像素）|
| 生成分辨率 | 建议 1024×1024，再经 §2 脚本压制 |

文件名沿用现有的 42 个（见 §9 逐卡条目），**不要改名**——`010_card_pool_v2.sql` 里的
`image_path` 与它们逐一对应，改名会让卡面变成空白且没有任何报错。

---

## 2. 流水线

模型直出的「像素风」几乎都是**假像素**：边缘带抗锯齿、色数上百、像素网格不对齐。
直接用会导致 42 张各有各的像素尺寸，放在一起像拼盘。

```
Kimi3 生成 1024×1024  →  scripts/cards/conform.py  →  256×256 成品
                          ├ 清除左下角「AI生成」灰度水印
                          ├ 降到 64×64 逻辑网格（最近邻采样）
                          ├ 自适应量化到 ≤24 色
                          └ 4× 最近邻放大到 256×256
```

用法 `python3 scripts/cards/conform.py <输入目录> -o <输出目录>`。

> **v1 的教训（2026-08-09）**：第一版压到 50×50 并把颜色吸附到「元素 5 阶 + 3 中性色」
> 共 8 色，结果 **12 张角色卡全毁**——黑甲、白炽剑芯、披风中间调统统被吸附到最近的
> 深红，明度对比一没，人物就塌成色块。6 张器物卡幸存，因为它们本就是单色调整体形。
>
> 原图画得很好，毁掉它们的是压制。跨卡一致性该由生成时的统一风格块（§3）保证，
> **不能让压制阶段用一个过窄的色板去硬凑**，那代价是画面本身。
>
> §5 的色阶表现在只作为**提示词里的取色指引**，不再是压制阶段的硬约束。

---

## 3. 通用风格块

**每一条提示词都以这段开头**，一字不改：

```
detailed 16-bit pixel art sprite, single centered subject, chunky readable pixels
on a 64x64 logical grid, hard-edged colour clusters with no anti-aliasing, limited
palette, hand-placed dithering for every gradient, light source from the upper left,
crisp one-pixel rim light along the lower-right contour, one-pixel outline in a
darker shade of the subject's own hue and never pure black, fully transparent
background, no ground, no frame, no border, no text, three-quarter orthographic
view, retro SNES monster-card illustration, crisp and legible at thumbnail size
```

## 4. 负面提示词

```
photorealistic, 3d render, octane, blurry, soft focus, anti-aliased edges, smooth
gradients, airbrush, jpeg artifacts, noise, text, letters, numbers, watermark,
signature, logo, UI elements, card frame, border, nameplate, drop shadow, white
background, grey background, checkerboard background, multiple subjects, collage,
cropped, out of frame, extra limbs, deformed anatomy, firearms, modern clothing,
existing franchise character, recognisable game mascot, anime screenshot
```

最后两条是硬约束，理由见 §8。

---

## 5. 元素色阶

取自 `generate_cards.py` 的元素色，向两端各扩一阶。**每张卡只用本元素的 5 阶 +
最多 3 个中性色**（木/皮革 `#8B5A2B`、金属 `#94A3B8`、骨白 `#E8E4D9`）。

| 元素 | 高光 | 亮 | 中 | 暗 | 描边 |
|---|---|---|---|---|---|
| 草 · 清风平原 | `#9EE6B8` | `#4ADE80` | `#22C55E` | `#15803D` | `#09451F` |
| 水 · 蓝水湖泊 | `#BDD8F9` | `#60A5FA` | `#3B82F6` | `#1D4ED8` | `#10359D` |
| 火 · 赤焰山脉 | `#FBCDD4` | `#FB7185` | `#EF4444` | `#B91C1C` | `#7F0F0F` |
| 雷 · 雷霆峡谷 | `#EEDFFC` | `#C084FC` | `#A855F7` | `#7E22CE` | `#591495` |
| 冰 · 永冬之巅 | `#C3F3F9` | `#67E8F9` | `#22D3EE` | `#0891B2` | `#025B71` |
| 岩 · 金石荒漠 | `#F6D685` | `#FBBF24` | `#F59E0B` | `#B45309` | `#733303` |

把本行的五个色值直接写进提示词，例如：
`palette built around #9EE6B8 #4ADE80 #22C55E #15803D #09451F`

**这是取色指引，不是硬上限。** 角色卡需要近黑的甲片与近白的高光来塑造形体，
把它们也压进元素色阶就会失去明度对比——v1 正是这样毁掉 12 张角色卡的。
压制阶段允许每张最多 24 色（§2）。

---

## 6. 稀有度分级

三档必须**一眼分得出来**，否则抽到传说卡没有感觉。差异不靠边框（边框是代码画的），
靠画面本身的信息密度。

| | 普通 24 张 | 稀有 12 张 | 传说 6 张 |
|---|---|---|---|
| 主体占画面 | 55–65% | 70–80% | 82–90% |
| 色数 | 5–7 | 8–11 | 12–16 |
| 姿态 | 静态、正面或侧面 | 3/4 动势、有重心偏移 | 对角线构图、有动作瞬间 |
| 特效 | 无，或一处极简高光 | 一层签名特效（火星/水雾/电弧） | 逆光 + 粒子 + 环境气息 |
| 追加提示词 | `simple flat silhouette, calm pose, minimal effects, 5 to 7 colours` | `ornate detail, dynamic three-quarter stance, one signature effect layer, rim lighting, 8 to 11 colours` | `epic dynamic diagonal composition, strong backlight, floating particles, faint environmental wisps, 12 to 16 colours` |

---

## 7. 剪影分配

**这是防单调的核心。** 同类卡最容易画成同一个形状——六张碎片全是菱形水晶，
换个颜色就交差。§9 里每张卡都带一行「剪影：」，指定它独占的外形，且已写进提示词。

三组最危险的同质区，以及各自的分化方案：

| 组 | 张数 | 分化 |
|---|---|---|
| 碎片 | 6 | 叶形 / 泪滴 / 不规则炭块 / 折线闪电 / 六棱柱簇 / 层状石板 |
| 生物 | 12 | 竖直人形 · 横向分节 · 圆胖侧视 · 竖向伞形 · 上尖火焰 · 粗壮 S 形 · 展翅三角 · 横宽云团 · 正圆带腿 · C 形蜷曲 · 低矮宽壳 · 直立双足 |
| 传说 | 6 | 盘绕 S · 跃起弧 · 上升 V · 后掠箭头 · 纯侧视 · 低矮横宽 |

器物六张靠**朝向**分化：束口袋（团）、螺壳（旋）、火把（竖向上）、电池（工业圆柱）、
冰锥（竖向下）、矿镐（对角）。同一档里刻意只有矿镐是斜的。

---

## 8. 授权红线

项目规定：**素材必须原创或公有领域，禁止任何商业游戏的角色名、立绘、贴图**。

- 提示词里不出现任何游戏、动画、IP 的名称，一个都没有
- 负面提示词固定带 `existing franchise character, recognisable game mascot`
- 生成后逐张过一眼：像某个知名角色的**重画**，即便提示词干净也要废弃重生成
- 42 张的 `source` 字段统一为 `原创生成 · AI 辅助 · CC0`，与 `SOURCES.md` 对齐

---

## 9. 42 条提示词

每条的完整提示词 = **§3 通用风格块** + **本条 SUBJECT** + **§6 对应档位的追加提示词** + **§5 对应元素的色阶行**。

---

### 普通 · 碎片（6 张）

**`common/grass_leaf_shard.png` — 翠叶碎片**
> 剪影：叶片形，带缺口与叶脉。不是水晶。

```
SUBJECT: a single translucent shard shaped like a broken leaf, jagged bite taken
out of one edge, glowing veins branching through the interior like leaf ribs,
floating upright at a slight tilt, three tiny motes drifting off the broken edge
```

**`common/water_water_drop.png` — 水珠碎片**
> 剪影：饱满泪滴，顶端收尖。表面要有一处高光弧。

```
SUBJECT: a single large water droplet held in mid-air, perfect teardrop profile
with a pointed top, one crescent highlight on the upper-left curve, faint caustic
ripples visible inside the body, a smaller satellite droplet below it
```

**`common/fire_ember_shard.png` — 火炭碎片**
> 剪影：不规则炭块，棱角粗粝。裂缝里透光。

```
SUBJECT: an irregular lump of charcoal with rough chipped facets, a network of
molten cracks glowing from within, the crack light strongest at the core and
fading toward the edges, two embers lifting off the top surface
```

**`common/thunder_spark_shard.png` — 电光碎片**
> 剪影：折线闪电形，全角度锐角，无一处曲线。

```
SUBJECT: a solidified lightning bolt fragment, sharp zigzag silhouette made only
of straight angular segments with no curves anywhere, faceted crystal surface,
a thin arc of electricity crawling along one edge
```

**`common/ice_ice_shard.png` — 冰晶碎片**
> 剪影：六棱柱簇，主柱 + 两根副柱。柱面要有平行的棱线。

```
SUBJECT: a cluster of three hexagonal ice prisms, one tall central column flanked
by two shorter ones at opposing angles, flat facet planes catching the light,
frost bloom creeping up from the base, a pale memory-like glow trapped deep inside
```

**`common/rock_sand_shard.png` — 砂石碎片**
> 剪影：扁平层状石板，横向堆叠。与前五张的立体感刻意相反。

```
SUBJECT: a flat slab of layered sandstone lying at a slight angle, horizontal
strata bands of alternating tone stacked through its thickness, one corner
crumbling into loose grains, worn rounded edges
```

---

### 普通 · 生物（12 张）

**`common/grass_sprout.png` — 芽苗精**
> 剪影：直立小人形，头顶两片叶。眼神要好奇。

```
SUBJECT: a tiny humanoid sprout spirit standing upright, a smooth bean-shaped body,
two broad leaves growing from the top of its head like a cap, stubby arms held out
in curiosity, wide bright eyes looking slightly upward, soil still clinging to its feet
```

**`common/grass_vine_bug.png` — 藤虫**
> 剪影：横向分节，波浪起伏。与芽苗精的竖直形成对比。

```
SUBJECT: a segmented caterpillar with a body made of woven vine, oriented
horizontally in a gentle wave, six small legs gripping a curling tendril, a trail
of three glowing footprints fading behind it, tiny leaf sprouting from its tail segment
```

**`common/water_bubble_fish.png` — 泡泡鱼**
> 剪影：圆胖侧视鱼，尾鳍张开。

```
SUBJECT: a round plump little fish seen from the side, oversized round eye, wide
fan tail, small pectoral fin, blowing a rising column of four bubbles of
decreasing size, a hint of a lakebed scene reflected inside the largest bubble
```

**`common/water_jelly_baby.png` — 水母仔**
> 剪影：竖向，钟形伞 + 飘散触须。与泡泡鱼的横向圆形拉开。

```
SUBJECT: a baby jellyfish, semi-transparent dome-shaped bell at the top, five
short wavy tentacles trailing downward and outward, soft inner glow visible
through the bell, drifting upward
```

**`common/fire_small_flame.png` — 小火苗**
> 剪影：向上收尖的火焰，有脸。

```
SUBJECT: a small sentient flame, teardrop body tapering to a flickering point at
the top, two simple round eyes and a cheerful curved mouth low on the body, tiny
arm-like wisps at the sides, one moth silhouette circling near the tip
```

**`common/fire_lava_worm.png` — 熔岩虫**
> 剪影：粗壮 S 形，甲片分段。

```
SUBJECT: a thick armoured worm curved into an S shape, overlapping dark carapace
plates along its back with molten orange seams glowing between them, blunt
eyeless head raised at the front, heat shimmer rising from its spine
```

**`common/thunder_static_bug.png` — 静电虫**
> 剪影：展翅甲虫，翅膀撑满上半部。

```
SUBJECT: a small beetle with two large translucent wings spread wide and raised,
compact rounded body below, thin antennae, tiny sparks jumping in the gap between
the two wings, delicate vein patterns across the wing membranes
```

**`common/thunder_cloud_baby.png` — 雷云仔**
> 剪影：横宽云团，下缘蓬松。与静电虫的展翅三角形相反。

```
SUBJECT: a small puffy storm cloud with a face, wide and low silhouette with a
bumpy scalloped underside, sulking expression with furrowed brows, one tiny
lightning bolt poking out from beneath it, small tuft curling up on top
```

**`common/ice_snow_ball.png` — 小雪球**
> 剪影：正圆 + 两只小短腿。全卡最简洁的形。

```
SUBJECT: a snowball creature, near-perfect sphere body with a slightly flattened
base, two stubby legs mid-stride, two small dot eyes near the top, a curling
trail of frost flowers left on the ground behind it
```

**`common/ice_frost_bug.png` — 霜虫**
> 剪影：C 形蜷曲，封在冰里。

```
SUBJECT: a pale larva curled into a C shape, sealed inside a block of clear ice
with rounded corners, its segmented body faintly visible through the ice, a
hairline crack running down one face of the block
```

**`common/rock_rock_beetle.png` — 岩甲虫**
> 剪影：低矮宽壳，俯视偏 3/4。壳面要有岩纹。

```
SUBJECT: a beetle with a thick stone shell, low and wide silhouette seen from a
high three-quarter angle, the shell surface cracked into rocky plates with a
mineral vein running across it, six short sturdy legs, small head barely emerging
from under the shell rim
```

**`common/rock_miner_rat.png` — 矿工鼠**
> 剪影：直立双足鼠，戴头盔。是这一档唯一的拟人角色。

```
SUBJECT: a bipedal rat miner standing upright, small leather helmet with a
glowing lamp on the front, one paw holding a tiny sack over its shoulder, long
thin tail curving behind for balance, alert whiskered snout, patched work vest
```

---

### 普通 · 器物（6 张）

**`common/grass_herb_pouch.png` — 草药袋**
> 剪影：束口布袋，鼓胀。

```
SUBJECT: a small cloth herb pouch with a drawstring cinched at the neck, body
bulging with contents, two sprigs of herb poking out of the opening, coarse woven
texture rendered as visible stitch-like pixel rows, a simple leaf emblem stitched
on the front
```

**`common/water_conch_shell.png` — 海螺壳**
> 剪影：螺旋，开口朝观者。

```
SUBJECT: a spiral conch shell resting at an angle, the spiral whorl clearly
readable with three visible turns, ribbed ridges following each turn, smooth
polished opening facing the viewer, pearlescent sheen on the inner lip
```

**`common/fire_torch.png` — 火把**
> 剪影：竖直木柄 + 顶端火焰。全卡最高的竖向形。

```
SUBJECT: a wooden torch held upright, wrapped cloth head burning with a tall
steady flame, bound leather grip on the shaft, three embers rising above the
flame, faint scorch marks on the wood below the wrapping
```

**`common/thunder_battery.png` — 电池**
> 剪影：圆柱体，顶端有正极凸起。工业感，与其他生物卡的有机形对比。

```
SUBJECT: a cylindrical energy cell standing on end, a raised terminal nub on top,
two metal bands around the body, a glowing charge-level window running vertically
down the front, small arc of electricity crossing the top terminal
```

**`common/ice_ice_spike.png` — 冰锥**
> 剪影：尖端向下的长锥。方向与火把刻意相反。

```
SUBJECT: a long tapered icicle hanging point-down, wide at the top and narrowing
to a needle tip, internal fracture lines catching the light, a single droplet
about to fall from the tip, frost crust across the thick upper portion
```

**`common/rock_pickaxe.png` — 矿镐**
> 剪影：对角线，是这一档唯一的斜向构图。

```
SUBJECT: a dwarven pickaxe laid on a strong diagonal, heavy forged double-sided
iron head, thick wooden haft bound with leather strapping at the grip, chips and
nicks along the striking edge, faint gold flecks embedded in the metal
```

---

### 稀有 · 守护者（6 张）

**`rare/grass_thorn_guard.png` — 荆棘守卫**
> 剪影：披甲战士，肩部荆棘外张形成宽肩。

```
SUBJECT: an elite warrior clad in living thorn armour, broad shoulders spiked with
outward-curving barbs, a round shield of layered bark held forward, helm woven
from vines with only the eyes glowing through, standing braced in a defensive
stance, loose petals drifting past
```

**`rare/water_tide_herald.png` — 潮汐使者**
> 剪影：长袍飘摆 + 举起的手，下摆化为浪。

```
SUBJECT: a robed herald of the tides, flowing garment whose lower hem dissolves
into a curling wave, one arm raised with the palm open commanding the water, a
crescent of suspended droplets orbiting the raised hand, hood shadowing a calm
face, sash streaming sideways
```

**`rare/fire_flame_knight.png` — 烈焰骑士**
> 剪影：持剑骑士，长剑斜举，火焰沿刃燃烧。

```
SUBJECT: a knight in blackened plate armour raising a longsword wreathed in flame,
the blade angled up and across the body, molten light glowing through the joints
and visor slit of the armour, tattered cape lifted by the heat, sparks streaming
off the sword edge
```

**`rare/thunder_storm_eye.png` — 风暴之眼**
> 剪影：兜帽观察者，胸前悬浮一颗眼球状球体。
>
> v1 出来是一团紫雾——「face is lost in shadow」等于允许模型什么都不画。
> 重写时给出**明确的三角兜帽轮廓**，并把浮球作为第二个可辨形状。

```
SUBJECT: a standing figure in a pointed hood, the hood forming a clear wide
triangle silhouette at the top, two bright slit eyes glowing inside the hood
opening, a long straight cloak falling to a flat hem, and one large round orb with
a dark iris floating just in front of the chest at arm's height. Thin lightning
arcs connect the orb to the figure's raised hand. Both the hood triangle and the
orb circle must stay clearly readable as separate shapes
```

**`rare/ice_frost_warden.png` — 霜冻卫士**
> 剪影：厚重铠甲哨兵，持长戟。表面刻满符文。

```
SUBJECT: a sentinel encased in thick glacial armour, angular pauldrons and greaves
carved with ancient runes that glow faintly from within the ice, gripping a long
frost halberd planted at its side, breath condensing into a small cloud, hoarfrost
spreading from where the weapon touches down
```

**`rare/rock_ridge_giant.png` — 山岭巨人**
> 剪影：宽厚驼背巨人，四肢短粗。是本档最大的体块。
>
> v1 出来是一堆石头——「body built from fitted boulders」被当成了主体描述。
> 重写时把**人形与四肢先说死**，材质降为修饰。

```
SUBJECT: a giant humanoid figure standing upright and facing the viewer, clearly
readable anatomy: one blocky head sunk between two enormous square shoulders, two
thick arms hanging past the knees with fists closed, two short pillar legs planted
apart. Its skin is grey stone with moss in the cracks, but the silhouette reads as
a person first and rock second. Deep-set glowing eyes under a heavy brow
```

---

### 稀有 · 神器（6 张）

**`rare/grass_life_seed.png` — 生命之种**
> 剪影：悬浮种子，顶端抽芽，外围一圈光环。

```
SUBJECT: a sacred seed floating in mid-air, smooth ovoid husk with a spiral groove,
a single delicate sprout unfurling from its crown, a thin ring of light encircling
it horizontally, motes of green light spiralling upward around the husk
```

**`rare/water_deep_pearl.png` — 深海珍珠**
> 剪影：张开的贝壳托着一颗珠。

```
SUBJECT: a great deep-sea shell hinged open, its ribbed halves framing a single
luminous pearl resting inside, iridescent nacre lining the shell interior, the
pearl casting light onto the shell walls, three small bubbles rising from the hinge
```

**`rare/fire_magma_heart.png` — 熔岩之心**
> 剪影：多面宝石，内部有跳动的核。

```
SUBJECT: a faceted gemstone with a molten core visible through its translucent
walls, sharp geometric facets catching hard highlights, the inner magma glowing
brightest at the centre and pulsing outward through veins in the stone, heat
distortion rising from the top facet
```

**`rare/thunder_thunder_hammer.png` — 雷霆之锤**
> 剪影：短柄重锤，锤头方正厚重。

```
SUBJECT: a dwarven war hammer standing head-up, massive rectangular hammer head
engraved with angular runes, short thick haft wrapped in worn leather, electricity
arcing between the two striking faces, a chip missing from one corner of the head
```

**`rare/ice_eternal_mirror.png` — 永冬之镜**
> 剪影：带柄圆镜，镜面结霜。

```
SUBJECT: an ornate hand mirror with a frost-rimed surface, circular frame of
twisted silver-blue metal with a short handle, the glass clouded with frost at the
edges and clearing toward the centre where a faint indistinct reflection shows,
ice crystals growing outward from the frame
```

**`rare/rock_gold_shield.png` — 金石护盾**
> 剪影：圆盾正面，中心有凸起盾心。

```
SUBJECT: a round shield forged from dense stone, seen face-on, a raised metal boss
at the centre with gold inlay radiating outward in geometric channels, the stone
surface scarred with deflected blows, riveted rim band, faint golden light in the
inlay grooves
```

---

### 传说 · 元素守护者（6 张）

**`legend/grass_guardian.png` — 翠灵龙**
> 剪影：东方龙盘绕成 S，鬃毛为叶。

```
SUBJECT: a serpentine eastern dragon coiled into a sweeping S curve, its mane and
whiskers formed from long trailing leaves, scales layered like overlapping petals,
antlered head turned toward the viewer, exhaling a stream of green breath that
sprouts tiny shoots where it passes, body wrapping into the depth of the frame
```

**`legend/water_guardian.png` — 潮汐鲸**
> 剪影：跃出水面的鲸，身体成弧，下方是浪。

```
SUBJECT: an immense whale breaching upward in a great arc, its underside pale and
ridged, water sheeting off its flanks in curtains, a wide ring of waves breaking
below where it left the surface, spray fanning behind the tail, one calm ancient
eye visible
```

**`legend/fire_guardian.png` — 炎凤**
> 剪影：展翅上升的凤凰，双翼向上张开成 V。

```
SUBJECT: a phoenix rising with both wings thrown open upward into a wide V, long
tail feathers streaming down in ribbons of flame, plumage transitioning from deep
ember at the body to bright fire at the feather tips, head thrown back mid-cry,
ash and sparks swirling in its updraft
```

**`legend/thunder_guardian.png` — 雷鹰**
> 剪影：俯冲的鹰，双翼后掠成箭头。与炎凤的上升 V 形相反。

```
SUBJECT: a great eagle in a steep dive, wings swept back tight against the airflow
forming an arrowhead silhouette, talons extended forward, feathers edged with
crackling electricity, a fork of lightning splitting the air along its dive path,
fierce forward-locked gaze
```

**`legend/ice_guardian.png` — 霜狼**
> 剪影：侧面仰头长嚎，尾部低垂。全卡唯一的纯侧视。

```
SUBJECT: a lone wolf in full profile, head raised in a howl, thick frost-laden
ruff around its neck, ice crystals forming along its spine and the tips of its
fur, breath rising as a visible plume, paws planted in snow with frost radiating
outward from each print
```

**`legend/rock_guardian.png` — 岩龟**
> 剪影：低矮宽厚，背甲隆起如山。是六张传说里最横向的一张。

```
SUBJECT: an ancient tortoise with a mountain growing from its shell, the carapace
rising into a rocky peak with tiny terraces and a thread of waterfall down one
side, thick pillar legs, deeply wrinkled neck and a slow patient gaze, worn
glyphs carved across the shell plates recording history
```

---

## 10. 验收

生成完成后逐项检查，任一不过就重生成：

- [ ] 42 张文件名与 §9 完全一致，尺寸 256×256 RGBA
- [ ] 背景全透明，没有边框 / 名牌 / 文字画进图里
- [ ] 缩略图（64px）下 42 张两两可区分——把它们排成一张联络表，一眼扫过去不能有「这两张是不是同一张」
- [ ] 每张色相以本元素为主，色数 ≤24
- [ ] 「AI生成」水印已清除，但出处仍记在 `cards.source` 与 `SOURCES.md`
- [ ] 三档稀有度的信息密度肉眼可分
- [ ] 没有任何一张像已有商业 IP 角色的重画
- [ ] `SOURCES.md` 已登记全部 42 张的来源与许可

最后一条别漏：`cards.source` 列的注释写着「spec F12 验收项：素材来源与许可证必须可追溯」。

---

## 11. 界面精灵图（9 张）

卡面之外还有九处在用 emoji 顶替：魔王四档形象、赛车、里程碑四枚奖牌。它们
跟应用的像素调性不搭，而且 emoji 在 Windows 与 macOS 上长得不一样——同一份
界面在目标平台上会是另一副样子。

规格与卡面一致（§1–§4 通用风格块 + 负面提示词照用），只有落盘位置与网格不同：

| 用途 | 逻辑网格 | 输出 | 显示尺寸 |
|---|---|---|---|
| 魔王四档 | 48 | 192 × 192 | 96px（正好 2:1） |
| 赛车 · 四枚奖牌 | 32 | 128 × 128 | 20–28px |

落盘 `wordcraft/public/assets/ui/`，压制：

```bash
python3 scripts/cards/conform.py <暂存目录> -g 48 -o wordcraft/public/assets/ui   # 魔王
python3 scripts/cards/conform.py <暂存目录> -g 32 -o wordcraft/public/assets/ui   # 其余
```

> **网格按题材复杂度选，不是按显示尺寸选。** 第一版这九张定了 16 格，
> 结果九张全毁——带鹿角的龙、带扰流板的赛车、带绶带的奖牌，16 个逻辑像素
> 根本放不下。原图是好的，毁在压制，与卡面 v1 完全同一类错误。

### 魔王四档

四档要**一眼看出强弱递进**，靠体型与轮廓复杂度，不靠单纯换色。

**`boss_tier1.png` — 遗忘小鬼**（lapses 1–2）
> 剪影：矮胖单角，比例接近球。四档里最小最圆。

```
SUBJECT: a small round imp with one stubby horn, oversized grin, tiny arms,
squat and harmless-looking, faint red glow around it
```

**`boss_tier2.png` — 记忆天狗**（lapses 3–4）
> 剪影：长鼻侧影 + 一对小翼，纵向拉长。

```
SUBJECT: a long-nosed goblin mask spirit in profile, small feathered wings folded
at its back, taller and leaner than the imp, ember flecks at the wingtips
```

**`boss_tier3.png` — 遗忘巨龙**（lapses 5–6）
> 剪影：盘绕蛇身 + 展开的角。占满画幅。

```
SUBJECT: a coiled serpentine dragon head-on, branching antlers spread wide,
sinuous body looping behind it, jaws parted with violet breath gathering
```

**`boss_tier4.png` — 深渊魔王**（lapses ≥ 7）
> 剪影：宽肩带角人形，双翼外张。四档里最宽最有压迫感。

```
SUBJECT: a broad-shouldered demon lord seen from the front, two great curved
horns, wings spread wide beyond the frame edges, molten cracks across the chest
armour, eyes burning
```

### 赛车

**`racer.png` — 赛车**
> 剪影：侧视流线车身。幽灵车复用同一张，代码里加灰度与透明度，不另出一张。

```
SUBJECT: a compact racing car in side profile, low streamlined body, large rear
wheel and smaller front wheel, small spoiler, motion streak trailing behind
```

### 里程碑奖牌

四枚要靠**形状**分级，不能只靠颜色——目标用户里可能有色觉差异，四枚只换色
就成了同一枚。

**`medal_bronze.png` — 初出茅庐**
> 剪影：圆牌 + 短绶带。最朴素。

```
SUBJECT: a simple round bronze medal with a plain rim, short ribbon folded above
it, a single small star stamped in the centre
```

**`medal_silver.png` — 渐入佳境**
> 剪影：圆牌 + 双层绶带 + 边缘齿纹。

```
SUBJECT: a round silver medal with a notched serrated rim, a two-layer ribbon
above it, two stars stamped side by side in the centre
```

**`medal_gold.png` — 持之以恒**
> 剪影：星形牌面而非圆形。这一档开始换外形。

```
SUBJECT: a five-pointed gold star medal, faceted surface catching light on the
upper-left points, ribbon threaded through the top point, laurel sprig curling
around the lower edge
```

**`crown.png` — 完美一周**
> 剪影：王冠，三个尖峰。全组唯一非牌面形状。

```
SUBJECT: a small ornate crown with three tapering peaks, a gemstone set at the
base of the centre peak, banded rim with a row of tiny studs, faint sparkle above
the tallest point
```

### 替换点

生成后按下表改引用，emoji 全部去掉：

| 文件 | 位置 |
|---|---|
| `src/components/BossBattle.tsx` | `getBossTheme` 的 `emoji` 字段改 `icon` 路径 |
| `src/components/SeasonTrack.tsx` | `MILESTONES[].icon`、两条赛道的 `🏎️` |
