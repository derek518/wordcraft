#!/usr/bin/env python3
"""从 ECDICT 提取高考考纲词表。

用法：
    python3 scripts/wordlist/extract.py <ecdict.csv> -o scripts/wordlist/words.json

数据源 skywind3000/ECDICT（MIT）。筛选 tag 含 gk 或 zk 的词条，产出契约
（contracts-v1.md §8）要求的字段。例句留空，由 T17 的生成管线补齐。

之所以用 Python 而非 TS：ECDICT 的 CSV 含大量嵌套引号与换行（释义字段里
有 \\n 和 \\r\\n），标准库 csv 处理这类转义比手写解析可靠得多。
"""

import argparse
import csv
import json
import re
import sys
from collections import Counter

# 契约 §8 的受控词性表。ECDICT 的词性缩写需映射到这里
VALID_POS = {
    "n.", "v.", "vt.", "vi.", "adj.", "adv.", "prep.", "conj.",
    "pron.", "art.", "num.", "int.", "aux.", "modal",
}

# 释义行首的词性标记，如 "vt. 放弃, 抛弃"
POS_PREFIX = re.compile(r"^\s*((?:[a-z]+\.)+|\[[^\]]+\])\s*")

# 学科标签行，如 "[计] 累加器" —— 专业释义对高中生无用且会挤掉常用义
DOMAIN_TAG = re.compile(r"^\s*\[[^\]]+\]")

# 释义中允许保留的字符：中文、常见标点。出现英文字母通常意味着字段错位
LATIN = re.compile(r"[A-Za-z]")


def normalize_pos(raw: str) -> str | None:
    """把 ECDICT 的词性缩写映射到受控词表。"""
    p = raw.strip().lower()
    if not p.endswith("."):
        p += "."
    # ECDICT 用 "a." 表示形容词、"ad." 表示副词
    alias = {"a.": "adj.", "ad.": "adv.", "prep.": "prep.", "conj.": "conj."}
    p = alias.get(p, p)
    return p if p in VALID_POS else None


def pos_candidates(exchange: str) -> set[str]:
    """从词形变化推断这个词能充当哪些词性。

    ECDICT 的 exchange 字段形如 `d:abandoned/p:abandoned/i:abandoning/3:abandons`：
        d/p/i/3  过去式、过去分词、现在分词、三单  → 可作动词
        s        复数                              → 可作名词
        r/t      比较级、最高级                     → 可作形容词或副词

    这是比释义行序更硬的证据。`statue` 只有 `s:statues` 而无动词变位，
    因此 ECDICT 里排在首位的 `vt. 以雕像装饰` 必然不是它的主用法。
    返回空集表示无形态学证据，此时只能退回行序。
    """
    kinds = {item.split(":", 1)[0] for item in exchange.split("/") if ":" in item}
    out: set[str] = set()
    if kinds & {"d", "p", "i", "3"}:
        out |= {"v.", "vt.", "vi."}
    if "s" in kinds:
        out.add("n.")
    if kinds & {"r", "t"}:
        out |= {"adj.", "adv."}
    return out


def parse_translation(translation: str, exchange: str = "") -> tuple[str | None, str]:
    """从中文释义中解析出主词性与清洗后的释义。

    ECDICT 的 translation 形如：
        "vt. 以雕像装饰\\nn. 雕像"
    多个词性用换行分隔，行首是词性缩写。**行序不代表常用度**，因此先用
    `exchange` 的形态学证据筛出可信的词性，只有在无证据时才退回行序。
    """
    lines = [ln.strip() for ln in re.split(r"\\n|\n|\r\n", translation) if ln.strip()]
    allowed = pos_candidates(exchange)

    parsed: list[tuple[str, str]] = []
    for line in lines:
        if DOMAIN_TAG.match(line):
            continue  # 整行是 [计] [化] 等专业释义

        m = POS_PREFIX.match(line)
        if not m:
            continue

        pos = normalize_pos(m.group(1))
        if pos is None:
            continue

        meaning = line[m.end():].strip()
        # 词性前缀之后仍可能带学科标签（如 `art. [计] 累加器`）。
        # 这类释义对高中生无用，整条丢弃而非只删标签——删了标签剩下的
        # 仍是「累加器、加法器」这种专业义
        if DOMAIN_TAG.match(meaning):
            continue

        meaning = re.sub(r"\s+", "", meaning)
        # 释义取前 3 个义项即可，长释义在四选一选项里显示不下
        parts = [p for p in re.split(r"[,，;；]", meaning) if p]
        if not parts:
            continue
        parsed.append((pos, "，".join(parts[:3])))

    if not parsed:
        return None, ""

    # 优先取形态学证据支持的词性
    for pos, meaning in parsed:
        if pos in allowed:
            return pos, meaning

    return parsed[0]


def frequency_band(bnc: int, frq: int) -> int:
    """按词频排名分 5 档，1 为最高频。

    取 bnc 与 frq 的较小值（即较高频的那个排名）：两个语料库各有盲区，
    BNC 偏英式书面语，当代语料库偏美式口语，取较高频者更接近"学生会遇到"的实际。
    排名为 0 表示该语料库未收录，此时用另一个。
    """
    ranks = [r for r in (bnc, frq) if r > 0]
    if not ranks:
        return 5  # 两个语料库都未收录 = 生僻词
    rank = min(ranks)
    if rank <= 1000:
        return 1
    if rank <= 3000:
        return 2
    if rank <= 6000:
        return 3
    if rank <= 12000:
        return 4
    return 5


def to_ipa(phonetic: str) -> str:
    """ECDICT 的音标不含斜杠，契约 §8 要求以 / 包裹。"""
    p = phonetic.strip()
    if not p:
        return ""
    p = p.strip("/[]").strip()
    return f"/{p}/" if p else ""


# 各区词数比例，取自 spec §5.2 的 50:200:300:500:500:500。
#
# 用比例而非绝对数字：spec 那张表总和 2050，而实际词库 3657——它是按一个更小的
# 词库假想画的。照搬绝对数字会剩下 1600 词无处安放；照搬「level × band 推导」
# 则格子大小完全由数据决定（junior∧band1-2 恰好 1271 词，规则无从控制）。
#
# 改为按难度排序后按比例切分：各区词数回到设计手里，难度梯度由排序保证。
ZONE_RATIO: list[tuple[str, int]] = [
    ("newbie", 1),
    ("grass", 4),
    ("water", 6),
    ("fire", 10),
    ("thunder", 10),
    ("ice", 10),
]

# 新手村固定 50 词（spec §5.2 的引导设计），不参与比例分配
NEWBIE_SIZE = 50


def assign_zones(words: list[dict]) -> None:
    """按难度顺序切分区域，原地写入 zone 字段。

    调用前 `words` 必须已按难度升序排列。
    """
    for w in words[:NEWBIE_SIZE]:
        w["zone"] = "newbie"

    rest = words[NEWBIE_SIZE:]
    ratios = ZONE_RATIO[1:]
    total_share = sum(share for _, share in ratios)

    start = 0
    for i, (zone, share) in enumerate(ratios):
        # 最后一区吃掉余数，避免整除误差留下未分配的词
        end = len(rest) if i == len(ratios) - 1 else start + len(rest) * share // total_share
        for w in rest[start:end]:
            w["zone"] = zone
        start = end


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("source", help="ecdict.csv 路径")
    ap.add_argument("-o", "--output", required=True, help="输出 JSON 路径")
    ap.add_argument(
        "--include-cet4",
        action="store_true",
        help="额外收入四级词（ECDICT 的 cet4 标签），标为 level=cet4。"
        "考纲之外的扩展，默认不收——高考备考期把四级词混进来会稀释重点",
    )
    args = ap.parse_args()

    csv.field_size_limit(sys.maxsize)

    words: list[dict] = []
    rejected: Counter[str] = Counter()
    seen: set[str] = set()

    with open(args.source, newline="", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            tags = (row.get("tag") or "").split()
            in_syllabus = "gk" in tags or "zk" in tags
            # 四级词只在显式要求时收，且不覆盖考纲内的分级——
            # 一个词既是高考词又是四级词时，它首先是高考词
            is_cet4 = args.include_cet4 and "cet4" in tags
            if not in_syllabus and not is_cet4:
                continue

            word = (row.get("word") or "").strip().lower()

            if not word or word in seen:
                rejected["重复或空词"] += 1
                continue
            # 契约 §8：^[a-z][a-z\-' ]*$
            if not re.fullmatch(r"[a-z][a-z\-' ]*", word):
                rejected["词形含非法字符"] += 1
                continue

            phonetic = to_ipa(row.get("phonetic") or "")
            if not phonetic:
                rejected["缺音标"] += 1
                continue

            pos, meaning = parse_translation(
                row.get("translation") or "", row.get("exchange") or ""
            )
            if pos is None:
                rejected["词性无法识别"] += 1
                continue
            if not meaning:
                rejected["释义为空"] += 1
                continue
            # 契约 §8：释义不得含英文字母（防字段错位）
            if LATIN.search(meaning):
                rejected["释义含英文字母"] += 1
                continue

            def as_int(key: str) -> int:
                try:
                    return int(row.get(key) or 0)
                except ValueError:
                    return 0

            band = frequency_band(as_int("bnc"), as_int("frq"))

            # zk 词属初中范围，其余高考词为高中。
            # 四级词单列一档：它在考纲之外，用户可以单独选择是否学
            if "zk" in tags:
                level = "junior"
            elif "gk" in tags:
                level = "senior"
            else:
                level = "cet4"
            edition = "both" if ("zk" in tags and "gk" in tags) else ("zk" if "zk" in tags else "gk")

            seen.add(word)
            words.append({
                "word": word,
                "phonetic": phonetic,
                "pos": pos,
                "meaning": meaning,
                "example_1": "",  # T17 生成
                "example_2": "",
                "level": level,
                "frequency_band": band,
                "source_edition": edition,
                "bnc": as_int("bnc"),
                "frq": as_int("frq"),
            })

    # 难度升序：先按词频档，同档内初中词在前，再按具体词频排名
    words.sort(key=lambda w: (
        w["frequency_band"],
        0 if w["level"] == "junior" else 1,
        min(r for r in (w["bnc"], w["frq"], 10**9) if r > 0),
    ))

    assign_zones(words)

    with open(args.output, "w", encoding="utf-8") as f:
        json.dump(words, f, ensure_ascii=False, indent=1)

    print(f"提取 {len(words):,} 词 → {args.output}")
    print()
    print("按 level：")
    for lv, n in Counter(w["level"] for w in words).most_common():
        print(f"  {lv:<8} {n:>6,}")
    print()
    print("按 frequency_band：")
    for band in sorted(Counter(w["frequency_band"] for w in words)):
        n = sum(1 for w in words if w["frequency_band"] == band)
        print(f"  band {band}   {n:>6,}")
    print()
    print("按 zone（难度升序）：")
    zc = Counter(w["zone"] for w in words)
    for zone, _ in ZONE_RATIO:
        members = [w for w in words if w["zone"] == zone]
        jr = sum(1 for w in members if w["level"] == "junior")
        print(f"  {zone:<9} {zc[zone]:>5}   初中词 {jr:>4} / 高中词 {len(members)-jr:>4}")
    if rejected:
        print()
        print("被拒条目：")
        for reason, n in rejected.most_common():
            print(f"  {reason:<16} {n:>6,}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
