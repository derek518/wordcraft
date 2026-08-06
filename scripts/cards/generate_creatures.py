#!/usr/bin/env python3
"""生成原创像素生物卡池。contracts §10.2 卡池 B。

程序化生成而非手绘：形态由参数组合产生，改配色或加形态只需动几行，
不必逐像素重画。所有素材为原创，无版权来源问题（§10.3）。

用法：
    python3 scripts/cards/generate_creatures.py
"""

import json
from pathlib import Path

from PIL import Image, ImageDraw

OUT_DIR = Path("wordcraft/public/cards/creatures")
MANIFEST = Path("scripts/cards/creatures.json")
GRID = 16          # 逻辑像素网格
SCALE = 12         # 每逻辑像素的物理大小 → 192x192

# 元素配色：(主色, 暗部, 高光)
ELEMENTS = {
    "grass": ((110, 200, 110), (60, 140, 70), (190, 240, 180)),
    "water": ((90, 160, 230), (45, 100, 175), (180, 220, 250)),
    "fire": ((235, 120, 70), (170, 60, 40), (255, 200, 140)),
    "thunder": ((190, 140, 240), (120, 80, 180), (230, 205, 255)),
    "ice": ((130, 220, 230), (70, 155, 175), (215, 250, 252)),
    "earth": ((190, 155, 110), (130, 95, 60), (230, 210, 170)),
}

DARK = (28, 30, 45, 255)
EYE = (250, 250, 255, 255)


def blob(d, cx, cy, r, fill, outline=None):
    """画一个圆润块。像素风里正圆会显得生硬，故用略扁的椭圆。"""
    box = [cx - r, cy - r * 0.85, cx + r, cy + r * 0.85]
    d.ellipse(box, fill=fill, outline=outline, width=1)


def draw_slime(d, pal, s):
    """史莱姆：底部宽、顶部圆，最基础的形态。"""
    main, dark, light = pal
    d.ellipse([2 * s, 6 * s, 14 * s, 14 * s], fill=main + (255,))
    d.ellipse([3 * s, 4 * s, 13 * s, 12 * s], fill=main + (255,))
    # 高光让它显得有体积而非一块平色
    d.ellipse([5 * s, 6 * s, 8 * s, 8 * s], fill=light + (180,))
    d.ellipse([4 * s, 12 * s, 12 * s, 15 * s], fill=dark + (255,))


def draw_flyer(d, pal, s):
    """飞行体：椭圆身体 + 上下分层的翼。

    早期版本用纯菱形，成品看着像箭头而不是生物——飞行动物的辨识特征
    是「圆身体 + 张开的翼」，翼要明显宽于身体。
    """
    main, dark, light = pal
    # 后翼（暗色，制造层次）
    d.polygon([(1 * s, 4 * s), (7 * s, 8 * s), (1 * s, 12 * s)], fill=dark + (255,))
    d.polygon([(15 * s, 4 * s), (9 * s, 8 * s), (15 * s, 12 * s)], fill=dark + (255,))
    # 前翼
    d.polygon([(2 * s, 6 * s), (7 * s, 9 * s), (3 * s, 13 * s)], fill=main + (255,))
    d.polygon([(14 * s, 6 * s), (9 * s, 9 * s), (13 * s, 13 * s)], fill=main + (255,))
    # 身体
    d.ellipse([6 * s, 5 * s, 10 * s, 13 * s], fill=main + (255,))
    d.ellipse([6.5 * s, 5.5 * s, 9.5 * s, 9 * s], fill=light + (150,))
    # 触角
    d.line([7 * s, 5 * s, 5.5 * s, 2 * s], fill=DARK, width=max(1, s // 3))
    d.line([9 * s, 5 * s, 10.5 * s, 2 * s], fill=DARK, width=max(1, s // 3))


# 元素特征：让同形态不同元素的卡也能一眼分辨。
# 只靠配色区分的话，六只四足兽看起来就是同一只换了色
BEAST_TRAITS = {
    "fire": "spikes",   # 背鳍
    "ice": "horns",     # 尖角
    "grass": "leaf",    # 头顶叶
    "thunder": "ears",  # 长耳
    "earth": "shell",   # 背甲
    "water": "fin",     # 尾鳍
}


def draw_beast(d, pal, s, element="earth"):
    """四足兽：躯干 + 四腿 + 元素特征。"""
    main, dark, light = pal
    trait = BEAST_TRAITS.get(element, "ears")

    # 躯干
    d.rounded_rectangle([3 * s, 7 * s, 12 * s, 12 * s], radius=s, fill=main + (255,))
    # 头
    blob(d, 12.5 * s, 6.5 * s, 3 * s, main + (255,))
    # 腿
    for x in (4, 6.5, 9, 11):
        d.rectangle([x * s, 12 * s, (x + 1.2) * s, 15 * s], fill=dark + (255,))
    # 腹部高光
    d.ellipse([5 * s, 8 * s, 10 * s, 10.5 * s], fill=light + (110,))

    if trait == "spikes":
        for x in (5, 7, 9):
            d.polygon([(x * s, 7 * s), ((x + 0.6) * s, 4.5 * s), ((x + 1.2) * s, 7 * s)],
                      fill=light + (255,))
    elif trait == "horns":
        d.polygon([(11 * s, 5 * s), (10.5 * s, 2 * s), (12.5 * s, 4.5 * s)], fill=light + (255,))
        d.polygon([(14 * s, 5 * s), (15 * s, 2 * s), (13 * s, 4.5 * s)], fill=light + (255,))
    elif trait == "leaf":
        d.ellipse([11.5 * s, 1.5 * s, 15 * s, 4.5 * s], fill=light + (255,))
        d.line([12.5 * s, 4.5 * s, 13 * s, 3 * s], fill=dark + (255,), width=max(1, s // 3))
    elif trait == "ears":
        d.ellipse([11 * s, 1.5 * s, 12.5 * s, 5 * s], fill=dark + (255,))
        d.ellipse([13.5 * s, 1.5 * s, 15 * s, 5 * s], fill=dark + (255,))
    elif trait == "shell":
        d.arc([3 * s, 4 * s, 12 * s, 13 * s], 180, 360, fill=dark + (255,), width=max(2, s // 2))
        d.arc([4.5 * s, 5.5 * s, 10.5 * s, 12 * s], 180, 360, fill=light + (200,),
              width=max(1, s // 3))
    elif trait == "fin":
        d.polygon([(3 * s, 9 * s), (0.5 * s, 5 * s), (1 * s, 12 * s)], fill=light + (255,))


def draw_crystal(d, pal, s):
    """水晶体：棱柱结构，与产品的「水晶」主题呼应。"""
    main, dark, light = pal
    d.polygon(
        [(8 * s, 1 * s), (13 * s, 7 * s), (10 * s, 15 * s), (6 * s, 15 * s), (3 * s, 7 * s)],
        fill=main + (255,),
    )
    d.polygon([(8 * s, 1 * s), (13 * s, 7 * s), (10 * s, 15 * s), (8 * s, 15 * s)],
              fill=dark + (255,))
    d.polygon([(8 * s, 2 * s), (6 * s, 7 * s), (8 * s, 6 * s)], fill=light + (220,))


SHAPES = {
    "slime": draw_slime,
    "flyer": draw_flyer,
    "beast": draw_beast,
    "crystal": draw_crystal,
}


def add_eyes(d, s, shape):
    """眼睛决定生物有没有「生命感」——纯色块看着像图标而非角色。"""
    if shape == "crystal":
        return
    # 眼睛尺寸放大到 1.4 格：像素图里 1 格的眼睛在缩略图中几乎消失，
    # 而眼睛正是「这是个生物」的关键信号
    y, x1, x2, r = {
        "slime": (8, 5.5, 9, 1.4),
        "flyer": (7, 6.8, 8.4, 1.0),
        "beast": (5.5, 11.5, 13.5, 1.3),
    }[shape]
    for x in (x1, x2):
        d.ellipse([x * s, y * s, (x + r) * s, (y + r) * s], fill=EYE)
        d.ellipse([(x + r * 0.25) * s, (y + r * 0.25) * s,
                   (x + r * 0.75) * s, (y + r * 0.75) * s], fill=DARK)


def add_rarity_marks(d, s, rarity, pal):
    """稀有度靠装饰体现，而不是只写个数字——玩家一眼要能分出高低。"""
    _, _, light = pal
    if rarity >= 2:
        # 稀有：环绕光点
        for x, y in [(2, 3), (14, 4), (3, 13), (13, 12)]:
            d.ellipse([x * s, y * s, x * s + s, y * s + s], fill=light + (230,))
    if rarity >= 3:
        # 传说：顶部王冠轮廓
        # 王冠压在头顶而非悬空——早期版本浮在图案上方，看着像误贴的贴纸
        d.polygon(
            [(5.5 * s, 4.2 * s), (6.8 * s, 2.2 * s), (8 * s, 4 * s),
             (9.2 * s, 2.2 * s), (10.5 * s, 4.2 * s)],
            fill=(255, 214, 102, 255),
        )
        d.rectangle([5.5 * s, 4 * s, 10.5 * s, 4.8 * s], fill=(255, 214, 102, 255))


# (名称, 形态, 元素, 稀有度, 冷知识)
#
# **(形态, 元素) 必须互不重复**：图案完全由这两项决定，组合撞车就是两张
# 一模一样的卡。此前「石背龟」与「沙丘鼠」同为 earth + beast，成品完全相同，
# 而这在代码里看不出来——只有把图排在一起才发现。下方 main() 有断言拦截。
CREATURES = [
    # 普通：每个元素一张，形态尽量分散
    ("草泥怪", "slime", "grass", 1, "史莱姆类生物的身体含水量超过 90%，与水母相当。"),
    ("水泡精", "slime", "water", 1, "水的表面张力能让小昆虫站在水面上行走。"),
    ("火苗兽", "beast", "fire", 1, "火焰的颜色取决于温度：红色约 800°C，蓝色可达 1400°C。"),
    ("石背龟", "beast", "earth", 1, "陆龟的甲壳由脊椎和肋骨演化融合而成，无法脱壳。"),
    ("嫩芽兔", "beast", "grass", 1, "兔子的视野接近 360 度，但正前方有盲区。"),
    ("寒露蝶", "flyer", "ice", 1, "蝴蝶用脚上的感受器尝味道，而不是用口器。"),
    ("电火虫", "flyer", "thunder", 1, "萤火虫的发光效率接近 100%，几乎不产生热量。"),
    ("熔岩泥", "slime", "fire", 1, "熔岩的黏度可以相差百万倍，取决于二氧化硅含量。"),
    # 稀有
    ("霜翼蛾", "flyer", "water", 2, "雪花有六重对称，源于水分子的氢键角度。"),
    ("雷角鹿", "beast", "thunder", 2, "闪电通道温度可达太阳表面的五倍。"),
    ("寒霜兽", "beast", "ice", 2, "北极狐的皮毛能在零下 40 度保持体温不流失。"),
    ("深潜灵", "slime", "thunder", 2, "海洋最深处的马里亚纳海沟压力超过一千个大气压。"),
    ("岩翼龙", "flyer", "earth", 2, "翼龙并非恐龙，它们属于独立的爬行动物支系。"),
    # 传说：全部水晶形态，靠元素配色区分
    ("星辉晶", "crystal", "thunder", 3, "石英晶体的压电效应是石英表走时精准的原理。"),
    ("永冻核", "crystal", "ice", 3, "冰有至少 19 种晶体结构，日常见到的只是其中一种。"),
    ("熔金石", "crystal", "fire", 3, "地球内核温度与太阳表面相当，约 5500°C。"),
]


def main() -> int:
    # 图案完全由 (形态, 元素) 决定，重复组合会产出一模一样的两张卡。
    # 在生成前拦截，而不是等人肉眼比对 16 张图
    combos = [(shape, element) for _, shape, element, _, _ in CREATURES]
    duplicates = {c for c in combos if combos.count(c) > 1}
    if duplicates:
        raise SystemExit(f"形态与元素组合重复，会生成完全相同的卡: {duplicates}")

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    manifest = []

    for idx, (name, shape, element, rarity, trivia) in enumerate(CREATURES, start=1):
        pal = ELEMENTS[element]
        img = Image.new("RGBA", (GRID * SCALE, GRID * SCALE), (0, 0, 0, 0))
        d = ImageDraw.Draw(img)

        if shape == "beast":
            draw_beast(d, pal, SCALE, element)
        else:
            SHAPES[shape](d, pal, SCALE)
        add_eyes(d, SCALE, shape)
        add_rarity_marks(d, SCALE, rarity, pal)

        filename = f"creature_{idx:02d}.png"
        img.save(OUT_DIR / filename)

        manifest.append({
            "name": name,
            "card_type": "creature",
            "rarity": rarity,
            "image_path": f"/cards/creatures/{filename}",
            "trivia": trivia,
            # §10.3：来源必须可追溯。原创素材注明生成脚本
            "source": "原创生成 · scripts/cards/generate_creatures.py · CC0",
        })

    MANIFEST.write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8"
    )

    by_rarity = {}
    for c in manifest:
        by_rarity[c["rarity"]] = by_rarity.get(c["rarity"], 0) + 1
    print(f"生成 {len(manifest)} 张生物卡 → {OUT_DIR}")
    for r in sorted(by_rarity):
        print(f"  稀有度 {r}: {by_rarity[r]} 张")
    print(f"清单 → {MANIFEST}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
