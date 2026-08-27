#!/usr/bin/env python3
"""合并词条与例句，产出可导入的词库（T18）。

用法：
    python3 scripts/wordlist/build_library.py

读 words.json（extract.py 产出）与 examples.json（gen_examples.py 产出），
按契约 §8 的 WordImportDto 校验后写出 wordcraft/src/data/library.json。

校验失败即拒绝并报告原因——**不静默跳过**。导入 3657 词时静默丢掉几百条，
要等用户发现某个词永远不出现才会暴露。
"""

import json
import re
import sys
from collections import Counter
from pathlib import Path

WORDS = Path("scripts/wordlist/words.json")
EXAMPLES = Path("scripts/wordlist/examples.json")
# 放 public/ 而非 src/：1MB 词库只在首次启动导入一次，
# 走 import 会被打进 JS bundle 长期占用内存，fetch 用完即弃
OUTPUT = Path("wordcraft/public/library.json")

VALID_POS = {
    "n.", "v.", "vt.", "vi.", "adj.", "adv.", "prep.", "conj.",
    "pron.", "art.", "num.", "int.", "aux.", "modal",
}
# cet4 为考纲外扩展，见 contracts §8
VALID_LEVELS = {"junior", "senior", "cet4", "art"}
VALID_ZONES = {"newbie", "grass", "water", "fire", "thunder", "ice", "rock"}

WORD_RE = re.compile(r"^[a-z][a-z\-' ]*$")
LATIN = re.compile(r"[A-Za-z]")

# spec §4：风格致敬可以，借用商业作品的专有名词不行。
#
# 生成时 prompt 已明确禁止，但模型在「游戏语境」提示下仍会偶尔说出这些名字，
# 所以构建阶段再拦一道——违规内容一旦进了词库，要等用户看见才发现。
BANNED_NAMES = [
    "Minecraft", "Steve", "Creeper", "Enderman", "Herobrine",
    "Genshin", "Paimon", "Zelda", "Link", "Mario", "Luigi",
    "Pokemon", "Pikachu", "Harry Potter", "Hogwarts", "Naruto",
    "Sonic", "Fortnite", "Roblox", "Among Us",
]


def find_banned(text: str, word: str) -> str | None:
    """检出受版权保护的专有名词。

    需排除词条自身：`link`（链接）与 `Link`（塞尔达角色）拼写相同，
    大小写不敏感的匹配无法区分，直接拦会误伤正常词条。
    """
    for name in BANNED_NAMES:
        if name.lower() == word.lower():
            continue
        if re.search(rf"\b{re.escape(name)}\b", text, re.IGNORECASE):
            return name
    return None


def contains_word(sentence: str, word: str) -> bool:
    """例句是否含该词的某个词形。

    与 gen_examples 的实现保持一致：取词干后要求词边界起始，
    避免 `art` 命中 `start` 这类误判。
    """
    stem = re.escape(word.split()[0][: max(3, len(word) - 3)])
    return re.search(rf"\b{stem}\w*", sentence, re.IGNORECASE) is not None


def validate(item: dict) -> str | None:
    """按契约 §8 校验，返回拒绝原因；通过则返回 None。"""
    w = item["word"]
    if not WORD_RE.fullmatch(w):
        return "词形不符合 ^[a-z][a-z\\-' ]*$"
    if not (item["phonetic"].startswith("/") and item["phonetic"].endswith("/")):
        return "音标未以 / 包裹"
    if item["pos"] not in VALID_POS:
        return f"词性 `{item['pos']}` 不在受控词表"
    if not item["meaning"]:
        return "释义为空"
    if LATIN.search(item["meaning"]):
        return "释义含英文字母"
    if not item["example_1"]:
        return "example_1 为空"
    if not contains_word(item["example_1"], w):
        return "example_1 不含该词的任何词形"
    banned = find_banned(f"{item['example_1']} {item['example_2']}", w)
    if banned:
        return f"例句含受版权保护的专有名词 `{banned}`"
    if item["frequency_band"] not in (1, 2, 3, 4, 5):
        return f"frequency_band `{item['frequency_band']}` 越界"
    if item["level"] not in VALID_LEVELS:
        return f"level `{item['level']}` 不在受控词表"
    if item["zone"] not in VALID_ZONES:
        return f"zone `{item['zone']}` 不在受控词表"
    return None


def main() -> int:
    words = json.loads(WORDS.read_text(encoding="utf-8"))
    examples = json.loads(EXAMPLES.read_text(encoding="utf-8"))

    library: list[dict] = []
    rejected: Counter[str] = Counter()
    samples: dict[str, str] = {}

    for w in words:
        ex = examples.get(w["word"], {})
        item = {
            "word": w["word"],
            "phonetic": w["phonetic"],
            "pos": w["pos"],
            "meaning": w["meaning"],
            "example_1": ex.get("example_1", ""),
            "example_2": ex.get("example_2", ""),
            "level": w["level"],
            "frequency_band": w["frequency_band"],
            "zone": w["zone"],
            "source_edition": w["source_edition"],
        }
        reason = validate(item)
        if reason:
            rejected[reason] += 1
            samples.setdefault(reason, item["word"])
            continue
        library.append(item)

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(
        json.dumps(library, ensure_ascii=False, separators=(",", ":")),
        encoding="utf-8",
    )

    print(f"词库 {len(library):,} 词 → {OUTPUT}  ({OUTPUT.stat().st_size/1024:.0f}KB)")
    print()
    print("按 zone：")
    for zone, n in Counter(x["zone"] for x in library).most_common():
        print(f"  {zone:<9} {n:>5,}")
    if rejected:
        print()
        print("被拒条目：")
        for reason, n in rejected.most_common():
            print(f"  {reason:<32} {n:>4}  例：{samples[reason]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
