#!/usr/bin/env python3
"""下载公有领域名画并像素化。contracts §10.2 卡池 A。

**许可核验由代码强制，不靠人工判断**：每张画先经 Wikimedia API 查询
`LicenseShortName`，不是公有领域就直接拒绝，绝不下载。作者逝世年份记错一次
就是一张侵权素材进库，这个判断不该交给记忆。

原图不入库（§10.3），只提交像素化后的成品。

用法：
    python3 scripts/cards/fetch_paintings.py
"""

import json
import time
import urllib.parse
import urllib.request
from pathlib import Path

from PIL import Image

API = "https://commons.wikimedia.org/w/api.php"
UA = "WordCraft/1.0 (educational vocabulary app; non-commercial)"
OUT_DIR = Path("wordcraft/public/cards/paintings")
MANIFEST = Path("scripts/cards/paintings.json")

# 像素化目标边长。48 比生物卡的 16 大，因为名画的辨识依赖构图细节，
# 压到 16 格后星月夜和睡莲会变成两坨相似的色块
PIXEL_SIZE = 48
OUTPUT_SIZE = 192

# 只接受这些许可标记。Wikimedia 的 PD 表述有多种写法，逐一列举而非
# 模糊匹配——`grep -i public` 会把 "Public domain in the US only" 也放进来
ACCEPTED_LICENSES = {
    "public domain",
    "cc0",
    "pd-art",
    "pd-old",
    "pd-old-100",
    "pd-us",
}

# (卡名, Wikimedia 文件名, 稀有度, 冷知识)
PAINTINGS = [
    ("星月夜", "Van Gogh - Starry Night - Google Art Project.jpg", 3,
     "梵高在圣雷米精神病院期间画下它，画中村庄是凭记忆虚构的。"),
    ("神奈川冲浪里", "Great Wave off Kanagawa2.jpg", 3,
     "画中远处的小山是富士山；这幅浮世绘曾影响德彪西创作交响诗《海》。"),
    ("呐喊", "Edvard Munch, 1893, The Scream, oil, tempera and pastel on cardboard, 91 x 73 cm, National Gallery of Norway.jpg", 3,
     "蒙克说灵感来自一次散步时「听见穿过自然的无尽呐喊」。"),
    ("戴珍珠耳环的少女", "1665 Girl with a Pearl Earring.jpg", 2,
     "研究认为那颗「珍珠」可能只是抛光的锡，真珍珠不会有这么大。"),
    ("向日葵", "Vincent Willem van Gogh 127.jpg", 2,
     "梵高画了至少 11 幅向日葵，用来装饰高更来访时的房间。"),
    ("大碗岛的星期天下午", "A Sunday on La Grande Jatte, Georges Seurat, 1884.jpg", 2,
     "修拉用点彩法画了两年，颜色由观者的眼睛在视网膜上混合。"),
    ("睡莲", "Claude Monet - Water Lilies - 1906, Ryerson.jpg", 1,
     "莫奈晚年患白内障，视野偏黄，这改变了他后期作品的色调。"),
    ("拾穗者", "Jean-François Millet (II) 013.jpg", 1,
     "拾穗是当时法律赋予穷人的权利：允许在收割后捡拾遗落的麦穗。"),
]


def with_retry(fn, attempts: int = 4, base_delay: float = 2.0):
    """重试网络操作。

    首轮实测遇到 429 限流与 SSL EOF，两者都是暂时性的——不重试就会因为
    一次抖动永久丢掉一张卡。指数退避，因为 429 恰恰说明请求太密。
    """
    last = None
    for i in range(attempts):
        try:
            return fn()
        except Exception as e:  # noqa: BLE001 - 网络错误类型繁多，一律重试
            last = e
            if i < attempts - 1:
                time.sleep(base_delay * (2 ** i))
    raise last


def api_query(filename: str) -> dict:
    params = {
        "action": "query",
        "titles": f"File:{filename}",
        "prop": "imageinfo",
        "iiprop": "url|size|extmetadata",
        "iiurlwidth": "640",
        "format": "json",
    }
    url = f"{API}?{urllib.parse.urlencode(params)}"
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=30) as resp:
        data = json.load(resp)

    pages = data.get("query", {}).get("pages", {})
    for page in pages.values():
        if "missing" in page:
            raise ValueError("Wikimedia 上不存在该文件")
        info = (page.get("imageinfo") or [None])[0]
        if not info:
            raise ValueError("响应中无 imageinfo")
        return info
    raise ValueError("响应中无页面数据")


def check_license(info: dict) -> str:
    """校验许可，返回许可名。非公有领域抛异常。"""
    md = info.get("extmetadata", {})
    license_name = md.get("LicenseShortName", {}).get("value", "")
    normalized = license_name.strip().lower()

    if normalized not in ACCEPTED_LICENSES:
        raise ValueError(
            f"许可 `{license_name}` 不在白名单内，拒绝使用。"
            f"白名单: {sorted(ACCEPTED_LICENSES)}"
        )
    return license_name


def pixelate(src: Path, dst: Path) -> None:
    """降采样再放大，得到硬边像素块。

    缩放两步走：先 LANCZOS 缩小（保留色彩分布），再 NEAREST 放大
    （保持像素边缘锐利）。直接 NEAREST 缩小会丢失大量色彩信息，
    成品发灰。
    """
    with Image.open(src) as im:
        im = im.convert("RGB")
        # 居中裁成正方形——卡面是方的，直接拉伸会让人物变形
        side = min(im.size)
        left = (im.width - side) // 2
        top = (im.height - side) // 2
        im = im.crop((left, top, left + side, top + side))

        small = im.resize((PIXEL_SIZE, PIXEL_SIZE), Image.LANCZOS)
        # 限制调色板，强化像素风格
        small = small.quantize(colors=32, method=Image.MEDIANCUT).convert("RGB")
        out = small.resize((OUTPUT_SIZE, OUTPUT_SIZE), Image.NEAREST)
        out.save(dst)


def main() -> int:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    tmp_dir = Path("/tmp/wordcraft_paintings")
    tmp_dir.mkdir(exist_ok=True)

    manifest = []
    rejected = []

    for idx, (name, filename, rarity, trivia) in enumerate(PAINTINGS, start=1):
        try:
            info = with_retry(lambda: api_query(filename))
            license_name = check_license(info)
        except Exception as e:
            rejected.append((name, str(e)))
            print(f"  ✗ {name}: {e}")
            continue

        thumb_url = info.get("thumburl") or info.get("url")
        page_url = info.get("descriptionurl", "")

        try:
            def fetch() -> bytes:
                req = urllib.request.Request(thumb_url, headers={"User-Agent": UA})
                with urllib.request.urlopen(req, timeout=60) as resp:
                    return resp.read()

            raw = with_retry(fetch)
            src = tmp_dir / f"src_{idx:02d}.jpg"
            src.write_bytes(raw)

            out_name = f"painting_{idx:02d}.png"
            pixelate(src, OUT_DIR / out_name)
        except Exception as e:
            rejected.append((name, f"下载或处理失败: {e}"))
            print(f"  ✗ {name}: {e}")
            continue

        manifest.append({
            "name": name,
            "card_type": "painting",
            "rarity": rarity,
            "image_path": f"/cards/paintings/{out_name}",
            "trivia": trivia,
            "source": f"Wikimedia Commons · {license_name} · {page_url}",
        })
        print(f"  ✓ {name} ({license_name}, {len(raw) // 1024}KB)")
        time.sleep(1.5)  # 0.5 秒会触发 429，实测需要更慢

    MANIFEST.write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8"
    )

    print(f"\n成功 {len(manifest)} 张 → {OUT_DIR}")
    if rejected:
        print(f"拒绝 {len(rejected)} 张：")
        for name, reason in rejected:
            print(f"  {name}: {reason[:90]}")
    print(f"清单 → {MANIFEST}")
    return 0 if manifest else 1


if __name__ == "__main__":
    raise SystemExit(main())
