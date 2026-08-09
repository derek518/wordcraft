#!/usr/bin/env python3
"""把生成的卡面压制成统一的像素规格。

    python3 scripts/cards/conform.py <输入目录> [-o 输出目录]

## v1 为什么失败（2026-08-09 复盘）

第一版把每张图压到 50×50、并把颜色吸附到「元素 5 阶 + 3 中性色」共 8 色。
结果是 **12 张角色卡全毁**：烈焰骑士的黑甲、剑刃的白炽芯、披风的中间调，
统统被吸附到最近的深红——明度对比一没，人物就塌成一坨色块。
6 张器物卡幸存，因为它们本就是单色调的整体形，丢内部细节不伤大局。

原图其实画得很好。毁掉它们的是压制，不是生成。三处改动：

1. **网格 50 → 64**。角色的解剖结构在 50 格里放不下
2. **BOX 平均 → 最近邻**。平均会把相邻的亮暗像素调和掉，正好抹平明度对比
3. **锁定 8 色 → 自适应 24 色**。跨卡一致性靠的是生成时的统一风格块，
   不该由压制阶段用一个过窄的色板硬凑；那代价是画面本身

## 水印

生成方在左下角烙了「AI生成」灰度水印。它不是画面的一部分，压制时清除；
**但 AI 生成这一事实必须留痕**——记在 `cards.source` 与 `SOURCES.md` 里
（见 docs/card-art-prompts.md §8）。抹掉图上的字不等于抹掉出处。
"""

import argparse
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    sys.exit("需要 Pillow：pip install Pillow")

# 逻辑像素网格与输出边长。256 = 64 × 4，整数倍——
# 非整数倍（如 200/64=3.125）放大后会出现 3px 与 4px 混杂的像素，
# 在像素画里一眼可见
GRID = 64
SCALE = 4
OUT_SIZE = GRID * SCALE

# 自适应色板上限。16–32 是像素画的常规区间：
# 再少会丢明度层次，再多则出现难以察觉的近似色、失去像素画的干净感
MAX_COLORS = 24

# 「AI生成」水印区域，按画布比例表示（左下角）
WATERMARK_BOX = (0.0, 0.90, 0.16, 1.0)
# 水印是纯灰度（实测饱和差为 0）。留一点余量，同时避免误伤金属/骨白等中性色
WATERMARK_MAX_SAT = 12

ALPHA_CUTOFF = 128


def strip_watermark(img):
    """清除左下角的灰度水印。

    双重判定：既要落在角落区域内，又要是低饱和灰——只按区域清会吃掉
    压在角上的画面，只按灰度清会吃掉画面里的金属与骨白。
    """
    w, h = img.size
    x0, y0, x1, y1 = WATERMARK_BOX
    box = (int(x0 * w), int(y0 * h), int(x1 * w), int(y1 * h))
    px = img.load()

    removed = 0
    for y in range(box[1], box[3]):
        for x in range(box[0], box[2]):
            r, g, b, a = px[x, y]
            if a == 0:
                continue
            if max(r, g, b) - min(r, g, b) <= WATERMARK_MAX_SAT:
                px[x, y] = (0, 0, 0, 0)
                removed += 1
    return removed


def to_grid(img):
    """降到逻辑网格。

    最近邻而非 BOX：像素画的相邻像素常常是刻意的明暗对照（甲片与高光），
    平均采样会把两者调和成中间调，正好毁掉塑造形体的那部分信息。
    """
    rgb = img.convert("RGB").resize((GRID, GRID), Image.NEAREST)
    alpha = img.getchannel("A").resize((GRID, GRID), Image.NEAREST)
    # 像素画没有半透明边缘，二值化掉抗锯齿残留
    alpha = alpha.point(lambda v: 255 if v >= ALPHA_CUTOFF else 0)
    out = rgb.convert("RGBA")
    out.putalpha(alpha)
    return out


def quantize(img):
    """自适应量化。透明像素不参与取色，否则背景色会占掉一个色位。"""
    rgb = img.convert("RGB")
    alpha = img.getchannel("A")
    q = rgb.quantize(colors=MAX_COLORS, method=Image.MEDIANCUT).convert("RGB")
    out = q.convert("RGBA")
    out.putalpha(alpha)
    return out


def conform(path, out_dir):
    img = Image.open(path).convert("RGBA")
    wm = strip_watermark(img)
    img = to_grid(img)
    img = quantize(img)
    img = img.resize((OUT_SIZE, OUT_SIZE), Image.NEAREST)

    out_dir.mkdir(parents=True, exist_ok=True)
    dest = out_dir / path.name
    img.save(dest)

    alpha = img.getchannel("A")
    opaque = sum(alpha.point(lambda v: 1 if v else 0).getdata())
    colors = len({p for p in img.convert("RGBA").getdata() if p[3] > 0})
    return dest, opaque / OUT_SIZE**2, colors, wm


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("input", type=Path, help="生成图所在目录")
    ap.add_argument("-o", "--output", type=Path, help="输出目录，默认 <输入>/conformed")
    args = ap.parse_args()

    if not args.input.is_dir():
        sys.exit(f"输入目录不存在: {args.input}")

    files = sorted(p for p in args.input.iterdir() if p.suffix.lower() == ".png")
    if not files:
        sys.exit(f"{args.input} 下没有 PNG")

    out_dir = args.output or args.input / "conformed"
    for path in files:
        dest, coverage, colors, wm = conform(path, out_dir)

        # 主体占比是构图的粗筛。火把、冰锥这类细长物天然偏低，
        # 所以只提示不判错——真正要人看的是过高：那通常是背景没抠干净
        flag = ""
        if coverage > 0.90:
            flag = "  ⚠ 几乎画满，疑似背景未抠除"
        elif coverage < 0.10:
            flag = "  ⚠ 主体过小"
        if wm == 0:
            flag += "  ⚠ 未发现水印（生成方是否换了标记方式？）"

        print(f"{dest.name:34} 占比 {coverage:5.1%}  {colors:2d}色  水印 {wm:5d}px{flag}")

    print(f"\n完成 {len(files)} 张 → {out_dir}（{OUT_SIZE}×{OUT_SIZE}）")


if __name__ == "__main__":
    main()
