#!/usr/bin/env python3
"""从 docs/card-art-prompts.md 拼装逐卡提示词，输出 TSV。

    python3 scripts/cards/build_prompts.py > scripts/cards/card_prompts.tsv

**提示词只有一个真相来源：那份 Markdown。** 手工维护一份平铺的 TSV，改了文档
却忘了重出，下一轮就会照着旧规格重生成——v1 的 50×50 网格与 8 色锁板正是
这样险些被重复使用。这个脚本让 TSV 永远是文档的派生物。

输出两列：目标文件名 · 完整提示词（通用风格块 + SUBJECT + 档位追加 + 色阶）。
"""

import re
import sys
from pathlib import Path

DOC = Path(__file__).resolve().parents[2] / "docs" / "card-art-prompts.md"

TIER_SUFFIX = {
    "common": "simple flat silhouette, calm pose, minimal effects, 5 to 7 colours",
    "rare": "ornate detail, dynamic three-quarter stance, one signature effect layer, "
    "rim lighting, 8 to 11 colours",
    "legend": "epic dynamic diagonal composition, strong backlight, floating particles, "
    "faint environmental wisps, 12 to 16 colours",
}


def flatten(text):
    return " ".join(text.split())


def extract_block(md, heading, fence_index=0):
    """取某个二级标题下的第 N 个代码块。"""
    start = md.index(f"## {heading}")
    nxt = md.find("\n## ", start + 1)
    section = md[start : nxt if nxt > 0 else len(md)]
    fences = re.findall(r"```\n(.*?)```", section, re.S)
    return flatten(fences[fence_index])


def main():
    if not DOC.exists():
        sys.exit(f"找不到 {DOC}")
    md = DOC.read_text()

    style = extract_block(md, "3. 通用风格块")
    negative = extract_block(md, "4. 负面提示词")

    # §5 色阶表：| 元素名 | 高光 | 亮 | 中 | 暗 | 描边 |
    ramps = {}
    for line in md.splitlines():
        m = re.match(r"\|\s*(草|水|火|雷|冰|岩)\s*·[^|]*\|(.+)\|", line)
        if m:
            key = {"草": "grass", "水": "water", "火": "fire",
                   "雷": "thunder", "冰": "ice", "岩": "rock"}[m.group(1)]
            ramps[key] = " ".join(re.findall(r"#[0-9A-F]{6}", m.group(2)))
    if len(ramps) != 6:
        sys.exit(f"色阶表解析失败，只取到 {len(ramps)} 个元素")

    # 逐卡条目：**`tier/name.png` — 中文名** 后面跟一个 SUBJECT 代码块
    # 剪影说明可能占多行引用（重写过的条目会附上失败原因），故用 (?:>[^\n]*\n)+
    entries = re.findall(
        r"\*\*`([a-z]+)/([a-z_0-9]+\.png)`[^\n]*\n(?:>[^\n]*\n)+\n```\n(SUBJECT: .*?)```",
        md,
        re.S,
    )
    if len(entries) != 42:
        sys.exit(f"逐卡条目解析出 {len(entries)} 条，应为 42——文档格式是否改过？")

    print("file\tprompt\tnegative")
    for tier, name, subject in entries:
        element = name.split("_")[0]
        prompt = (
            f"{style}, {flatten(subject).removeprefix('SUBJECT: ')}, "
            f"{TIER_SUFFIX[tier]}, palette built around {ramps[element]}"
        )
        print(f"{tier}/{name}\t{prompt}\t{negative}")


if __name__ == "__main__":
    main()
