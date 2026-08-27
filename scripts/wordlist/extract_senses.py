#!/usr/bin/env python3
"""从 ECDICT 抽出词库各词的**全部**释义行，供 gen_meanings.py 挑选。

用法：
    curl -o /tmp/ecdict.csv https://raw.githubusercontent.com/skywind3000/ECDICT/master/ecdict.csv
    python3 scripts/wordlist/extract_senses.py /tmp/ecdict.csv

产出 senses.json（约 800KB，入库），这样后续步骤不必再拉那 66MB 源文件。

## 为什么要留全部释义行

`extract.py` 只保留了它挑中的那一行，而它的挑法有系统性偏差：先用 exchange
的词形变化筛词性，再取该词性的第一行——而 ECDICT 的行序是词典编排顺序，
不是常用度。实测前 130 个高频词里约四分之一挑错：

    can    vt. 装罐          （aux. 能, 可以 在第 3 行）
    still  n. 蒸馏室         （adv. 仍然 在第 4 行）
    may    n. 五月           （aux. 可以 在第 2 行）
    must   n. 未发酵葡萄汁    （aux. 必须 在第 2 行）

而且 19% 的词 exchange 为空，完全没有形态学证据可用，只能盲取第一行。

正确释义**就在数据里**，只是没排在前面。所以这一步不做判断，只把候选原样
留下，判断交给 gen_meanings.py。
"""

import csv
import json
import re
import sys
from pathlib import Path

LIBRARY = Path("wordcraft/public/library.json")
OUTPUT = Path("scripts/wordlist/senses.json")

# 行首的词性缩写，如 `vt. 装罐` / `a. 平坦的`
POS_PREFIX = re.compile(r"^([a-zA-Z]+\.)\s*")


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2

    wanted = {w["word"] for w in json.loads(LIBRARY.read_text(encoding="utf-8"))}
    csv.field_size_limit(10**7)

    out: dict[str, list[str]] = {}
    with open(sys.argv[1], newline="", encoding="utf-8") as f:
        for row in csv.DictReader(f):
            if row["word"] not in wanted:
                continue
            lines = [
                ln.strip()
                for ln in re.split(r"\\n|\n|\r\n", row["translation"])
                if ln.strip() and POS_PREFIX.match(ln.strip())
            ]
            if lines:
                out[row["word"]] = lines

    OUTPUT.write_text(
        json.dumps(out, ensure_ascii=False, indent=1, sort_keys=True),
        encoding="utf-8",
    )
    missing = len(wanted) - len(out)
    size = OUTPUT.stat().st_size / 1024
    print(f"候选释义 {len(out):,} 词 → {OUTPUT}  ({size:.0f}KB)")
    if missing:
        print(f"⚠️  {missing} 个词在 ECDICT 中没有可解析的释义行，将沿用现有释义")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
