#!/usr/bin/env python3
"""合并词条与例句，产出可导入的词库（T18）。

用法：
    python3 scripts/wordlist/build_library.py

读 words.json（extract.py 产出）、examples.json（gen_examples.py 产出）与
meanings.json（gen_meanings.py 产出），按契约 §8 的 WordImportDto 校验后写出
wordcraft/public/library.json。

meanings.json 覆盖 words.json 的 `pos` / `meaning`：`extract.py` 按 ECDICT 的
**行序**挑释义，而那是词典编排顺序不是常用度，实测前 130 个高频词约四分之一
挑错（can→装罐、still→蒸馏室）。没被覆盖的词沿用原释义。

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
MEANINGS = Path("scripts/wordlist/meanings.json")
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

# 高频词的词性哨兵。
#
# 这些词在 ECDICT 里都有一个排在前面、但极少用的义项（can 的「装罐」、
# still 的「蒸馏室」、may 的「五月」）。按行序挑必然踩中，实测前 130 个高频词
# 约四分之一如此。释义重新生成时若又挑回去，这里当场报错——
# 否则要等孩子背错了才会有人发现。
SPOT_CHECKS = {
    "can": "aux.", "may": "aux.", "must": "aux.", "will": "aux.",
    "should": "aux.", "would": "aux.", "could": "aux.",
    "still": "adv.", "just": "adv.", "even": "adv.", "well": "adv.",
    "but": "conj.", "leave": "vt.",
}

# 必须带第二词性的词。
#
# 这些词的两个用法在高考里都高频，只教一个等于把考点删掉一半：
# watch 是「看」也是「手表」，train 是「火车」也是「训练」，
# right 是「正确的」也是「权利」。重新生成时若丢了第二词性，这里当场报错。
SECOND_POS_REQUIRED = ["watch", "train", "right", "light", "park", "plant", "firm", "share"]

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


def unify_separators(text: str | None) -> str | None:
    """义项分隔符统一成全角「，」。

    模型有时照抄 ECDICT 的半角 `, `，有时自己规范成全角——实测 1,829 词半角、
    2,438 词全角。四选一的四个选项并排显示时，混用一眼就能看出来。

    在校验**之后**做：校验要求义项原样抄自源文，先规范化会让子串匹配失效。
    """
    if not text:
        return text
    return re.sub(r"\s*,\s*", "，", text).strip()


def frequency_rank(w: dict) -> int | None:
    """全局词频排名，取 BNC 与当代语料库中较高频的那个。

    这是**难度轴**：能力模型按它推断「这个词孩子会不会」。
    `frequency_band` 是它压成 5 档的产物，5278 个词分 5 桶太粗——
    一个桶上千个词，无法区分 the 和排在第 900 名的词。

    两个语料库都未收录时返回 None，不做插补。这 18 个词多是
    连字符复合词（ice-cream / father-in-law / cd-rom），编一个排名
    会让能力模型把凭空捏造的难度当成证据。宁可标为未知。
    """
    ranks = [r for r in (w.get("bnc", 0), w.get("frq", 0)) if r and r > 0]
    return min(ranks) if ranks else None


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
    p2, m2 = item.get("pos_2"), item.get("meaning_2")
    if (p2 is None) != (m2 is None):
        return "pos_2 与 meaning_2 必须同时有或同时无"
    if p2 is not None:
        if p2 == item["pos"]:
            return f"pos_2 `{p2}` 与主词性相同"
        if not m2 or len(m2) > 20:
            return f"meaning_2 长度非法（{len(m2 or '')} 字）"

    rank = item["frequency_rank"]
    if rank is not None and (not isinstance(rank, int) or rank < 1):
        return f"frequency_rank `{rank}` 非法（应为正整数或 null）"
    if item["level"] not in VALID_LEVELS:
        return f"level `{item['level']}` 不在受控词表"
    if item["zone"] not in VALID_ZONES:
        return f"zone `{item['zone']}` 不在受控词表"
    return None


def main() -> int:
    words = json.loads(WORDS.read_text(encoding="utf-8"))
    examples = json.loads(EXAMPLES.read_text(encoding="utf-8"))
    meanings = (
        json.loads(MEANINGS.read_text(encoding="utf-8")) if MEANINGS.exists() else {}
    )
    refined = 0

    library: list[dict] = []
    rejected: Counter[str] = Counter()
    samples: dict[str, str] = {}

    for w in words:
        ex = examples.get(w["word"], {})
        # 挑过的释义优先。挑不出来（或被校验拒绝）的词沿用 extract.py 的原值，
        # 而不是留空——缺释义的词根本没法出题
        picked = meanings.get(w["word"])
        if picked:
            refined += 1
        item = {
            "word": w["word"],
            "phonetic": w["phonetic"],
            "pos": picked["pos"] if picked else w["pos"],
            "meaning": picked["meaning"] if picked else w["meaning"],
            # 第二词性：只有模型判定「高考阅读里大概率会遇到」的才有。
            # null 就是没有，不用空串伪装成有
            "pos_2": (picked or {}).get("pos2"),
            "meaning_2": (picked or {}).get("meaning2"),
            "example_1": ex.get("example_1", ""),
            "example_2": ex.get("example_2", ""),
            "level": w["level"],
            "frequency_band": w["frequency_band"],
            "frequency_rank": frequency_rank(w),
            "zone": w["zone"],
            "source_edition": w["source_edition"],
        }
        reason = validate(item)
        # 规范化排在校验之后：校验比对的是源文，先改分隔符会让子串匹配失效
        item["meaning"] = unify_separators(item["meaning"])
        item["meaning_2"] = unify_separators(item["meaning_2"])
        if reason:
            rejected[reason] += 1
            samples.setdefault(reason, item["word"])
            continue
        library.append(item)

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    by_word = {x["word"]: x for x in library}
    bad = [
        f"{w}: 期望 {want}，实得 {by_word[w]['pos']} {by_word[w]['meaning']}"
        for w, want in SPOT_CHECKS.items()
        if w in by_word and by_word[w]["pos"] != want
    ]
    missing_2 = [w for w in SECOND_POS_REQUIRED if w in by_word and not by_word[w].get("pos_2")]
    if bad or missing_2:
        if bad:
            print("词性哨兵未通过——这些高频词又挑回了生僻义：", file=sys.stderr)
            for b in bad:
                print(f"  {b}", file=sys.stderr)
        if missing_2:
            print("这些词的第二词性丢了——两个用法在高考里都高频：", file=sys.stderr)
            for w in missing_2:
                print(f"  {w}: 只有 {by_word[w]['pos']} {by_word[w]['meaning']}", file=sys.stderr)
        need = sorted(set(w for w in bad for w in [w.split(":")[0]]) | set(missing_2))
        print("\n重跑 gen_meanings.py --words " + " ".join(need or SPOT_CHECKS), file=sys.stderr)
        return 1

    OUTPUT.write_text(
        json.dumps(library, ensure_ascii=False, separators=(",", ":")),
        encoding="utf-8",
    )

    print(f"词库 {len(library):,} 词 → {OUTPUT}  ({OUTPUT.stat().st_size/1024:.0f}KB)")
    stale = len(library) - refined
    with_2 = sum(1 for x in library if x.get("pos_2"))
    print(f"释义：已重挑 {refined:,}，沿用原值 {stale:,}，带第二词性 {with_2:,}")
    if stale:
        print("  （沿用的是 extract.py 按行序挑的，可能不是最常用义——重跑 gen_meanings.py 补齐）")
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
