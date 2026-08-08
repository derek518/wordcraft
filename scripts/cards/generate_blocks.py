#!/usr/bin/env python3
"""生成家园建造方块素材。plan: docs/plans/homestead-v1.1.md（H4）。

原有三张不可用：block_normal 是个纯白方块，block_rare 是几根竖条纹，
都不像能拿来搭建的东西。这里重画。

用等距（isometric）视角而非正视：家园是要「堆」出来的，等距块并排放置时
天然有立体感，正视方块只会看起来像一张色卡表。

用法：
    python3 scripts/cards/generate_blocks.py
"""

import math
from pathlib import Path

from PIL import Image, ImageDraw

OUT_DIR = Path("wordcraft/public/assets/blocks")
SIZE = 128          # 输出边长
PIXEL = 4           # 像素块大小，制造硬边颗粒感


def iso_points(cx: float, cy: float, w: float, h: float, depth: float):
    """等距立方体的三个面。返回 (顶面, 左面, 右面) 的多边形顶点。"""
    top = [
        (cx, cy - h / 2),
        (cx + w / 2, cy),
        (cx, cy + h / 2),
        (cx - w / 2, cy),
    ]
    left = [
        (cx - w / 2, cy),
        (cx, cy + h / 2),
        (cx, cy + h / 2 + depth),
        (cx - w / 2, cy + depth),
    ]
    right = [
        (cx + w / 2, cy),
        (cx, cy + h / 2),
        (cx, cy + h / 2 + depth),
        (cx + w / 2, cy + depth),
    ]
    return top, left, right


def shade(color, factor):
    """按系数调整明度。三个面用同一底色的不同明度，立体感就出来了。"""
    return tuple(min(255, max(0, int(c * factor))) for c in color[:3]) + (255,)


def pixelate(img: Image.Image) -> Image.Image:
    """降采样再放大，把平滑边缘变成像素块——与项目其余素材的风格一致。"""
    small = img.resize((SIZE // PIXEL, SIZE // PIXEL), Image.NEAREST)
    return small.resize((SIZE, SIZE), Image.NEAREST)


def speckle(d: ImageDraw.ImageDraw, points, base, seed: int, count: int, factor: float):
    """在多边形内撒噪点。纯色面看着像塑料，噪点让它有材质。

    用确定性的伪随机而非 random：素材要可复现，每次生成结果必须一致。
    """
    xs = [p[0] for p in points]
    ys = [p[1] for p in points]
    x0, x1, y0, y1 = min(xs), max(xs), min(ys), max(ys)

    state = seed
    for _ in range(count):
        state = (state * 1103515245 + 12345) & 0x7FFFFFFF
        px = x0 + (state % 1000) / 1000 * (x1 - x0)
        state = (state * 1103515245 + 12345) & 0x7FFFFFFF
        py = y0 + (state % 1000) / 1000 * (y1 - y0)
        # 点在菱形内的粗略判定：离中心的曼哈顿距离
        cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
        if abs(px - cx) / ((x1 - x0) / 2 + 0.1) + abs(py - cy) / ((y1 - y0) / 2 + 0.1) > 0.9:
            continue
        d.rectangle([px, py, px + PIXEL, py + PIXEL], fill=shade(base, factor))


def draw_block(base, *, speckled=True, glow=None, rainbow=False, seed=7):
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    cx, cy = SIZE / 2, SIZE * 0.36
    w, h, depth = SIZE * 0.72, SIZE * 0.42, SIZE * 0.30
    top, left, right = iso_points(cx, cy, w, h, depth)

    if glow:
        # 外发光：稀有方块要一眼看出不同
        for i in range(6, 0, -1):
            g = ImageDraw.Draw(img)
            spread = i * 2
            gt, gl, gr = iso_points(cx, cy, w + spread, h + spread * 0.6, depth + spread * 0.4)
            alpha = int(18 * (7 - i) / 6)
            for poly in (gt, gl, gr):
                g.polygon(poly, fill=glow + (alpha,))

    if rainbow:
        # 限定方块：顶面走彩虹渐变，逐条画而非整面填充
        steps = 12
        for i in range(steps):
            t = i / steps
            hue = t * 300
            r = int(127 + 127 * math.cos(math.radians(hue)))
            g_ = int(127 + 127 * math.cos(math.radians(hue - 120)))
            b = int(127 + 127 * math.cos(math.radians(hue - 240)))
            y0 = cy - h / 2 + h * t
            y1 = cy - h / 2 + h * (t + 1 / steps)
            span = (1 - abs(2 * (t - 0.5))) * w / 2
            d.polygon(
                [(cx - span, y0), (cx + span, y0), (cx + span, y1), (cx - span, y1)],
                fill=(r, g_, b, 255),
            )
    else:
        d.polygon(top, fill=shade(base, 1.25))

    d.polygon(left, fill=shade(base, 0.62))
    d.polygon(right, fill=shade(base, 0.88))

    if speckled:
        speckle(d, top, base, seed, 26, 1.10)
        speckle(d, left, base, seed + 1, 20, 0.54)
        speckle(d, right, base, seed + 2, 20, 0.78)

    # 棱线。没有它三个面会糊成一团
    edge = shade(base, 0.40)
    for poly in (top, left, right):
        d.line(poly + [poly[0]], fill=edge, width=2)

    return pixelate(img)


BLOCKS = {
    # 普通：石灰岩质感，密集噪点。这是最常见的方块，不能抢眼
    "block_normal": dict(base=(150, 152, 168), speckled=True, seed=11),
    # 稀有：紫水晶 + 外发光。spec 要求「发光、特殊纹理」
    "block_rare": dict(base=(150, 90, 220), speckled=True, glow=(180, 120, 255), seed=23),
    # 限定：彩虹玻璃，spec 点名的样式
    "block_limited": dict(base=(90, 180, 200), speckled=False, rainbow=True,
                          glow=(120, 220, 235), seed=37),
}


def main() -> int:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    for name, kw in BLOCKS.items():
        img = draw_block(**kw)
        img.save(OUT_DIR / f"{name}.png")
        print(f"  ✓ {name}.png  {img.size}")

    # 旧的 block_special 被 block_limited 取代
    old = OUT_DIR / "block_special.png"
    if old.exists():
        old.unlink()
        print("  - 删除 block_special.png（由 block_limited 取代）")

    print(f"\n{len(BLOCKS)} 个方块 → {OUT_DIR}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
