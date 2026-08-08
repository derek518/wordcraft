#!/usr/bin/env python3
"""用 Edge-TTS 预生成单词发音。contracts §3.5 / spec F4。

系统 TTS 已经可用（T19），预生成解决的是另外两件事：音质（神经网络语音
明显优于系统合成）与一致性（所有用户听到同一个发音）。缓存缺失时应用会
自动降级回系统 TTS，所以这一步是优化而非必需。

**产物不入 git**：3657 个 mp3 约 38MB，二进制文件会让仓库永久膨胀，且每次
重新生成都产生全新 blob。音频是构建产物，打包时生成。

用法：
    .venv/bin/python scripts/tts/pregenerate.py                # 全量
    .venv/bin/python scripts/tts/pregenerate.py --band 1 2     # 只做高频词
    .venv/bin/python scripts/tts/pregenerate.py --concurrency 6
"""

import argparse
import asyncio
import json
import time
from pathlib import Path

import edge_tts

LIBRARY = Path("wordcraft/public/library.json")
# 放 src-tauri/ 而非 public/：public 下的文件会被 Vite 复制进 dist
# 再嵌入二进制，与 Tauri resources 各存一份，白白多出 44MB
OUT_DIR = Path("wordcraft/src-tauri/audio")

# en-US-AriaNeural：清晰、语速适中的美音。教材与高考听力以美音为主
VOICE = "en-US-AriaNeural"
RATE = "-10%"  # 略慢于常速，照顾基础薄弱的学习者


async def synth(word: str, path: Path, retries: int = 3) -> tuple[str, bool, str]:
    """合成单词发音。返回 (词, 是否成功, 错误信息)。"""
    for attempt in range(retries):
        try:
            await edge_tts.Communicate(word, VOICE, rate=RATE).save(str(path))
            # 空文件也算失败：Edge-TTS 偶尔返回 200 但内容为空，
            # 落到磁盘上是个 0 字节 mp3，播放时才发现没声音
            if path.stat().st_size < 512:
                raise ValueError(f"产出文件过小（{path.stat().st_size} 字节）")
            return word, True, ""
        except Exception as e:  # noqa: BLE001 - 网络与服务端错误类型繁多
            if attempt < retries - 1:
                await asyncio.sleep(1.5 * (2 ** attempt))
            else:
                path.unlink(missing_ok=True)
                return word, False, f"{type(e).__name__}: {e}"
    return word, False, "未知"


async def run(words: list[str], concurrency: int) -> tuple[int, list[tuple[str, str]]]:
    sem = asyncio.Semaphore(concurrency)
    done = 0
    failures: list[tuple[str, str]] = []
    total = len(words)

    async def worker(word: str):
        nonlocal done
        async with sem:
            result = await synth(word, OUT_DIR / f"{word.lower()}.mp3")
            done += 1
            if done % 100 == 0 or done == total:
                print(f"  {done}/{total}")
            return result

    for word, ok, err in await asyncio.gather(*[worker(w) for w in words]):
        if not ok:
            failures.append((word, err))
    return total - len(failures), failures


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--band", type=int, nargs="*", help="只生成指定频段")
    ap.add_argument("--concurrency", type=int, default=8)
    ap.add_argument("--force", action="store_true", help="重新生成已存在的")
    args = ap.parse_args()

    if not LIBRARY.exists():
        print(f"词库不存在：{LIBRARY}\n先运行 scripts/wordlist/build_library.py")
        return 1

    library = json.loads(LIBRARY.read_text(encoding="utf-8"))
    if args.band:
        library = [w for w in library if w["frequency_band"] in args.band]

    OUT_DIR.mkdir(parents=True, exist_ok=True)

    words = []
    skipped = 0
    for entry in library:
        w = entry["word"]
        target = OUT_DIR / f"{w.lower()}.mp3"
        # 断点续传：已有且非空的跳过。中断后重跑不必从头来
        if not args.force and target.exists() and target.stat().st_size >= 512:
            skipped += 1
            continue
        words.append(w)

    print(f"词库 {len(library)} 词，已有 {skipped}，待生成 {len(words)}")
    if not words:
        print("无待生成项。")
        return 0
    print(f"音色 {VOICE}，语速 {RATE}，并发 {args.concurrency}")

    started = time.time()
    ok, failures = asyncio.run(run(words, args.concurrency))
    elapsed = time.time() - started

    size_mb = sum(f.stat().st_size for f in OUT_DIR.glob("*.mp3")) / 1024 / 1024
    print(f"\n成功 {ok}，失败 {len(failures)}，耗时 {elapsed / 60:.1f} 分钟")
    print(f"音频总计 {size_mb:.0f}MB → {OUT_DIR}")

    if failures:
        print(f"\n失败词（重跑本脚本会自动重试）：")
        for word, err in failures[:10]:
            print(f"  {word}: {err[:80]}")
        if len(failures) > 10:
            print(f"  …另有 {len(failures) - 10} 个")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
