#!/usr/bin/env python3
"""从 ECDICT 的候选释义里挑出高中生该记的那一条。

用法：
    export DEEPSEEK_API_KEY=...        # 或写进项目根的 .env
    python3 scripts/wordlist/gen_meanings.py --limit 40    # 先小批试跑
    python3 scripts/wordlist/gen_meanings.py               # 全量

## 这是选择题，不是生成题

模型只能从 senses.json 给出的释义行里挑，**校验强制返回的每个义项都是原文
子串**。挑错了顶多是选了个次要义项，编不出词库里没有的东西——这是本项目
「禁止硬编码/伪造释义」那条红线在数据管线上的落实。

## 为什么需要它

`extract.py` 先按 exchange 的词形变化筛词性、再取该词性第一行，而 ECDICT 的
行序是词典编排顺序而非常用度。实测前 130 个高频词约四分之一挑错（can→装罐、
still→蒸馏室、may→五月、must→未发酵葡萄汁），另有 29% 的词在行内被截断到
3 个义项，把最常用的那个切掉了（survey 的「调查」排第 5）。

76% 的词有多个词性行，手写规则修不干净：aux. 优先能解决 can/may/must，但
still/well/even/down 的名词义在形态学上完全合法，只能一个个打补丁。

## 增量与续跑

每批立即落盘。5176 词跑十几分钟，中途限流不落盘就得整批重来。重启时跳过
meanings.json 里已有的词。
"""

import argparse
import json
import os
import re
import sys
import threading
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

API_BASE = "https://api.deepseek.com"
DEFAULT_MODEL = "deepseek-v4-flash"

SENSES = Path("scripts/wordlist/senses.json")
OUTPUT = Path("scripts/wordlist/meanings.json")

VALID_POS = {
    "n.", "v.", "vt.", "vi.", "adj.", "adv.", "prep.", "conj.",
    "pron.", "art.", "num.", "int.", "aux.",
}

# ECDICT 用 `a.` 表示形容词、`ad.` 表示副词，模型照抄候选行里的写法是**对的**，
# 归一是调用方的责任。先前直接拿去比对受控词表，把一批本来挑对了的词判成失败
ALIAS = {"a.": "adj.", "ad.": "adv.", "adj": "adj.", "adv": "adv.", "n": "n.", "v": "v."}


def normalize_pos(raw: str) -> str:
    p = raw.strip().lower()
    if p and not p.endswith("."):
        p += "."
    return ALIAS.get(p, p)

PROMPT = """你在为中国高中生的单词记忆软件整理释义。

我会给你一批单词，每个附带词典里的**全部**释义行。请为每个词挑出高中生该记的
主要用法。

规则：
1. `pos` 从这些里选一个：n. v. vt. vi. adj. adv. prep. conj. pron. art. num. int. aux.
   - 情态动词（can/may/must/will/should）一律选 aux.
   - 优先选这个词**最常用**的词性，不是词典排在第一的那个
2. `meaning` 给 2-4 个义项，用「，」分隔
   - **每个义项必须原样抄自我给的释义行**，不许改写、不许自己造词
   - 按常用度排序，最常用的放第一个
   - 可以跨行取：比如 still 选 adv. 时取「仍然」，不必受行内顺序限制
   - 丢掉专业术语义（[计]、[医]、[化] 等）和生僻义
3. 释义要短。四选一的选项框放不下长句

只返回 JSON，不要任何解释：
{"单词": {"pos": "词性", "meaning": "义项1，义项2"}, ...}"""


def api_key() -> str:
    key = os.environ.get("DEEPSEEK_API_KEY", "").strip()
    if key:
        return key
    env = Path(".env")
    if env.exists():
        for line in env.read_text(encoding="utf-8").splitlines():
            if line.startswith("DEEPSEEK_API_KEY="):
                return line.split("=", 1)[1].strip().strip("'\"")
    sys.exit(
        "未找到 DEEPSEEK_API_KEY。\n"
        "    export DEEPSEEK_API_KEY=...\n"
        "  或  echo 'DEEPSEEK_API_KEY=你的密钥' > .env"
    )


def request(path: str, payload: dict | None, key: str, timeout: int = 120) -> dict:
    data = json.dumps(payload).encode() if payload is not None else None
    req = urllib.request.Request(
        API_BASE + path,
        data=data,
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"},
        method="POST" if data else "GET",
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read())


def normalize(text: str) -> str:
    """去掉空白与标点差异，用于子串校验。"""
    return re.sub(r"[\s,，;；.。()（）]", "", text)


def validate(word: str, item: dict, candidates: list[str]) -> str | None:
    """返回错误原因；None 表示通过。"""
    pos = normalize_pos(str(item.get("pos", "")))
    meaning = str(item.get("meaning", "")).strip()
    if pos not in VALID_POS:
        return f"词性 `{pos}` 不在受控词表"
    if not meaning:
        return "释义为空"

    parts = [p for p in re.split(r"[,，;；]", meaning) if p.strip()]
    if not (1 <= len(parts) <= 4):
        return f"义项数 {len(parts)} 越界（应为 1-4）"

    # 每个义项都必须能在候选行里找到。模型只有挑选权，没有创作权——
    # 编造的释义混进词库，要等孩子背错了才会有人发现
    pool = normalize("".join(candidates))
    for p in parts:
        if normalize(p) not in pool:
            return f"义项 `{p}` 不在候选释义中（疑似编造）"

    if len(meaning) > 24:
        return f"释义过长（{len(meaning)} 字），四选一选项框放不下"
    return None


def pick_batch(batch: list[tuple[str, list[str]]], model: str, key: str) -> dict[str, dict]:
    listing = "\n".join(f"{w}: {' | '.join(lines)}" for w, lines in batch)
    payload = {
        "model": model,
        "messages": [
            {"role": "system", "content": PROMPT},
            {"role": "user", "content": listing},
        ],
        "temperature": 0.1,
        "response_format": {"type": "json_object"},
    }
    resp = request("/chat/completions", payload, key)
    content = resp["choices"][0]["message"]["content"]
    return json.loads(content)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", default=DEFAULT_MODEL)
    ap.add_argument("--batch", type=int, default=18)
    ap.add_argument("--workers", type=int, default=6)
    ap.add_argument("--limit", type=int, help="只处理前 N 个词，用于试跑")
    ap.add_argument("--words", nargs="*", help="只处理指定的词")
    args = ap.parse_args()

    key = api_key()
    senses: dict[str, list[str]] = json.loads(SENSES.read_text(encoding="utf-8"))
    done: dict[str, dict] = (
        json.loads(OUTPUT.read_text(encoding="utf-8")) if OUTPUT.exists() else {}
    )

    if args.words:
        todo = [(w, senses[w]) for w in args.words if w in senses]
    else:
        todo = [(w, lines) for w, lines in sorted(senses.items()) if w not in done]
        if args.limit:
            todo = todo[: args.limit]

    if not todo:
        print("没有待处理的词")
        return 0

    batches = [todo[i : i + args.batch] for i in range(0, len(todo), args.batch)]
    print(f"待处理 {len(todo):,} 词，分 {len(batches)} 批，模型 {args.model}")

    lock = threading.Lock()
    stats = {"ok": 0, "rejected": 0}
    rejections: list[str] = []

    def run(batch: list[tuple[str, list[str]]]) -> None:
        for attempt in range(3):
            try:
                picked = pick_batch(batch, args.model, key)
                break
            except (urllib.error.HTTPError, urllib.error.URLError, TimeoutError) as e:
                if attempt == 2:
                    with lock:
                        rejections.append(f"整批失败: {e}")
                    return
                time.sleep(2 ** attempt * 3)
            except (KeyError, json.JSONDecodeError) as e:
                with lock:
                    rejections.append(f"响应无法解析: {e}")
                return

        accepted = {}
        for word, lines in batch:
            item = picked.get(word)
            if not isinstance(item, dict):
                with lock:
                    stats["rejected"] += 1
                    rejections.append(f"{word}: 未返回")
                continue
            reason = validate(word, item, lines)
            if reason:
                with lock:
                    stats["rejected"] += 1
                    rejections.append(f"{word}: {reason}")
                continue
            accepted[word] = {
                "pos": normalize_pos(str(item["pos"])),
                "meaning": str(item["meaning"]).strip(),
            }

        # 每批立即落盘：中途限流不落盘就得整批重来，token 白花
        with lock:
            done.update(accepted)
            stats["ok"] += len(accepted)
            OUTPUT.write_text(
                json.dumps(done, ensure_ascii=False, indent=1, sort_keys=True),
                encoding="utf-8",
            )
            print(f"\r已完成 {stats['ok']:,} / {len(todo):,}  拒绝 {stats['rejected']}", end="", flush=True)

    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        futures = [pool.submit(run, b) for b in batches]
        for f in as_completed(futures):
            f.result()

    print(f"\n通过 {stats['ok']:,}，拒绝 {stats['rejected']}")
    if rejections:
        print("\n拒绝原因（前 20 条）：")
        for r in rejections[:20]:
            print(f"  {r}")
        print("\n被拒的词沿用 extract.py 的原释义。重跑本脚本会只处理它们。")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
