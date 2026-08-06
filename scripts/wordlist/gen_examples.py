#!/usr/bin/env python3
"""为词库批量生成例句（T17）。

用法：
    export DEEPSEEK_API_KEY=...          # 不要写进代码或提交入库
    python3 scripts/wordlist/gen_examples.py --list-models
    python3 scripts/wordlist/gen_examples.py --zone newbie grass
    python3 scripts/wordlist/gen_examples.py                      # 全量

设计要点：
- **每批立即落盘**。3657 词要跑半小时，中途限流或中断若不增量写盘就得整批重来，
  token 白花。重启时跳过 examples.json 里已有的词。
- 生成结果逐条校验「例句含该词的某个词形」（契约 §8），不合格的退回重生成。
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

WORDS_PATH = Path("scripts/wordlist/words.json")
OUTPUT_PATH = Path("scripts/wordlist/examples.json")

# spec §4 的素材策略：语境取自游戏/创作场景以贴近目标用户，
# 但禁止任何商业作品的专有名词——风格致敬可以，借用角色名不行
PROMPT = """你在为中国高中生的单词记忆软件生成英文例句。

要求：
1. 每个单词生成 2 条例句，长度 6-14 词，语法正确，用词不超出高中范围
2. 例句必须包含该单词本身或其变形（复数、时态等）
3. 语境从以下四类中选取，让例句有画面感：沙盒建造游戏、奇幻冒险、赛车竞速、绘画创作
4. **禁止出现任何真实存在的商业游戏、动漫、影视中的角色名、地名、专有名词**
   —— 风格可以致敬，名字必须原创或通用
5. 句子要自然，不要为了塞单词而生硬

按 JSON 数组返回，不要任何解释文字：
[{"word": "单词", "example_1": "第一句", "example_2": "第二句"}]

需要生成的单词："""


def load_words(zones: list[str] | None) -> list[dict]:
    words = json.loads(WORDS_PATH.read_text(encoding="utf-8"))
    if zones:
        words = [w for w in words if w["zone"] in zones]
    return words


def load_done() -> dict[str, dict]:
    if OUTPUT_PATH.exists():
        return json.loads(OUTPUT_PATH.read_text(encoding="utf-8"))
    return {}


def save_done(done: dict[str, dict]) -> None:
    """原子写盘：先写临时文件再 rename。

    直接覆盖写有风险——若在 json.dump 中途被杀，文件就是半截 JSON，
    下次启动解析失败，已完成的几千词进度全丢，比不做断点续传更糟。
    POSIX 保证 rename 原子，读到的要么是旧版本要么是新版本。
    """
    tmp = OUTPUT_PATH.with_suffix(".tmp")
    tmp.write_text(json.dumps(done, ensure_ascii=False, indent=1), encoding="utf-8")
    os.replace(tmp, OUTPUT_PATH)


def api_key() -> str:
    """从环境变量或项目根的 .env 读取密钥。

    支持 .env 是因为脚本可能被逐次调用（每次都是新 shell），
    export 的变量不会保留。.env 已在 .gitignore 中。
    """
    key = os.environ.get("DEEPSEEK_API_KEY", "").strip()

    if not key:
        env_file = Path(".env")
        if env_file.exists():
            for line in env_file.read_text(encoding="utf-8").splitlines():
                line = line.strip()
                if line.startswith("DEEPSEEK_API_KEY="):
                    key = line.split("=", 1)[1].strip().strip("'\"")
                    break

    if not key:
        sys.exit(
            "未找到 DEEPSEEK_API_KEY。\n"
            "在项目根目录创建 .env（已被 gitignore）：\n"
            "    echo 'DEEPSEEK_API_KEY=你的密钥' > .env\n"
            "或设置同名环境变量。切勿把密钥写进代码或提交入库。"
        )
    return key


def request(path: str, payload: dict | None, key: str, timeout: int = 180) -> dict:
    data = json.dumps(payload).encode() if payload is not None else None
    req = urllib.request.Request(
        f"{API_BASE}{path}",
        data=data,
        headers={
            "Authorization": f"Bearer {key}",
            "Content-Type": "application/json",
        },
        method="POST" if data else "GET",
    )
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        return json.loads(resp.read().decode())


def list_models(key: str) -> None:
    try:
        result = request("/models", None, key, timeout=30)
    except urllib.error.HTTPError as e:
        sys.exit(f"查询模型列表失败（HTTP {e.code}）：{e.read().decode()[:200]}")
    print("可用模型：")
    for m in result.get("data", []):
        print(f"  {m.get('id')}")


def contains_word(sentence: str, word: str) -> bool:
    """例句是否包含该词的某个词形（契约 §8）。

    宽松匹配词干：`abandon` 可以以 abandoned / abandoning 形式出现。
    直接子串匹配会误判（`art` 命中 `start`），故要求词边界起始。
    """
    stem = re.escape(word.split()[0][:max(3, len(word) - 3)])
    return re.search(rf"\b{stem}\w*", sentence, re.IGNORECASE) is not None


def extract_json(text: str) -> list[dict]:
    """模型有时会用 ``` 包裹或加前言，取第一个 JSON 数组。"""
    m = re.search(r"\[.*\]", text, re.DOTALL)
    if not m:
        raise ValueError(f"响应中未找到 JSON 数组: {text[:200]}")
    return json.loads(m.group(0))


def generate_batch(batch: list[dict], model: str, key: str) -> dict[str, dict]:
    listing = "\n".join(f"- {w['word']} ({w['pos']} {w['meaning']})" for w in batch)
    payload = {
        "model": model,
        "messages": [{"role": "user", "content": PROMPT + "\n" + listing}],
        "temperature": 1.0,
        "max_tokens": 4000,
        # deepseek-v4-flash 默认开启推理，且思考与输出共用 max_tokens 预算。
        # 实测有批次把 8000 tokens 全花在 reasoning 上、输出一个字都没剩，
        # 报错还伪装成「未找到 JSON 数组」。
        #
        # 生成例句是模板化产出——约束明确、无需权衡，推理纯属浪费。关闭后
        # reasoning_tokens 归零，既不会截断也更省。
        #
        # 注意参数名必须准确：`enable_thinking: false` 会被静默忽略（实测
        # reasoning 仍有 478 tokens），API 不会为未知字段报错。
        "thinking": {"type": "disabled"},
    }
    result = request("/chat/completions", payload, key)
    choice = result["choices"][0]

    # 先看 finish_reason 再解析——token 耗尽时的报错必须指向真实原因，
    # 否则排查会被「JSON 解析失败」带偏
    if choice.get("finish_reason") == "length":
        usage = result.get("usage", {})
        reasoning = usage.get("completion_tokens_details", {}).get("reasoning_tokens", 0)
        raise ValueError(
            f"输出被 max_tokens 截断（completion={usage.get('completion_tokens')}, "
            f"其中 reasoning={reasoning}），请减小 --batch-size"
        )

    content = choice["message"]["content"]

    out: dict[str, dict] = {}
    wanted = {w["word"] for w in batch}
    for item in extract_json(content):
        word = str(item.get("word", "")).strip().lower()
        if word not in wanted:
            continue
        e1 = str(item.get("example_1", "")).strip()
        e2 = str(item.get("example_2", "")).strip()
        # 契约 §8：example_1 必须含该词。不合格的留给下一轮重生成，
        # 不写入——写了就等于把坏数据固化进词库
        if not e1 or not contains_word(e1, word):
            continue
        out[word] = {"example_1": e1, "example_2": e2 if contains_word(e2, word) else ""}
    return out


def run_round(
    pending: list[dict],
    args: argparse.Namespace,
    key: str,
    done: dict[str, dict],
    lock: threading.Lock,
) -> list[dict]:
    """并发跑完一轮，返回失败待重试的词。"""
    batches = [
        pending[i : i + args.batch_size]
        for i in range(0, len(pending), args.batch_size)
    ]
    failed: list[dict] = []
    completed = 0

    def work(batch: list[dict]) -> tuple[list[dict], dict[str, dict]]:
        try:
            return [], generate_batch(batch, args.model, key)
        except urllib.error.HTTPError as e:
            body = e.read().decode()[:120]
            # 限流要退避，否则并发只会把 429 打得更密
            time.sleep(20 if e.code == 429 else 5)
            print(f"    HTTP {e.code}: {body}")
            return batch, {}
        except Exception as e:  # noqa: BLE001 - 网络与解析错误都要能续跑
            time.sleep(3)
            print(f"    {type(e).__name__}: {e}")
            return batch, {}

    with ThreadPoolExecutor(max_workers=args.concurrency) as pool:
        futures = {pool.submit(work, b): b for b in batches}
        for fut in as_completed(futures):
            batch = futures[fut]
            bad, got = fut.result()

            # 锁同时保护 dict 更新与落盘：两者必须一致，
            # 否则崩溃时磁盘上的进度可能落后于内存
            with lock:
                done.update(got)
                save_done(done)
                completed += 1
                total = len(done)

            failed.extend(bad)
            failed.extend(w for w in batch if w["word"] not in got and w not in bad)
            print(f"  [{completed:>3}/{len(batches)}] +{len(got):>2}/{len(batch)}  累计 {total:,}")

    return failed


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", default=DEFAULT_MODEL)
    ap.add_argument("--zone", nargs="*", help="只生成指定分区")
    ap.add_argument("--batch-size", type=int, default=20)
    ap.add_argument("--concurrency", type=int, default=6, help="并发请求数")
    ap.add_argument("--list-models", action="store_true")
    ap.add_argument("--max-rounds", type=int, default=3, help="失败词的重试轮数")
    args = ap.parse_args()

    key = api_key()

    if args.list_models:
        list_models(key)
        return 0

    words = load_words(args.zone)
    done = load_done()
    lock = threading.Lock()

    pending = [w for w in words if w["word"] not in done]
    print(f"目标 {len(words):,} 词，已完成 {len(words) - len(pending):,}，待生成 {len(pending):,}")
    print(f"模型 {args.model}，每批 {args.batch_size} 词，并发 {args.concurrency}")
    if not pending:
        print("无待生成词条。")
        return 0

    started = time.time()
    for round_no in range(1, args.max_rounds + 1):
        if not pending:
            break
        print(f"\n─── 第 {round_no} 轮，{len(pending):,} 词 ───")
        pending = run_round(pending, args, key, done, lock)

    elapsed = time.time() - started
    if pending:
        print(f"\n仍有 {len(pending)} 词未生成：{', '.join(w['word'] for w in pending[:20])}")
    print(f"\n完成 {len(done):,} 词，耗时 {elapsed/60:.1f} 分钟 → {OUTPUT_PATH}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
