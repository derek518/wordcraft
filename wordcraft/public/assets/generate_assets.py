#!/usr/bin/env python3
"""
WordCraft Asset Generator
生成像素风格的素材图片
"""
import os
from PIL import Image, ImageDraw

OUTPUT_DIR = "/Users/derek/Codes/高考英语/wordcraft/public/assets"

def ensure_dirs():
    for subdir in ["crystals", "portals", "blocks", "badges", "ui", "effects"]:
        os.makedirs(os.path.join(OUTPUT_DIR, subdir), exist_ok=True)

def create_pixel_image(pixel_grid, palette, scale=8):
    """
    根据像素网格生成图片
    pixel_grid: 2D list of color keys
    palette: dict mapping keys to (R,G,B,A) tuples
    scale: 每个逻辑像素的物理像素大小
    """
    h = len(pixel_grid)
    w = len(pixel_grid[0]) if h > 0 else 0
    img = Image.new("RGBA", (w * scale, h * scale), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    
    for y, row in enumerate(pixel_grid):
        for x, key in enumerate(row):
            if key in palette and palette[key][3] > 0:
                draw.rectangle(
                    [x * scale, y * scale, (x + 1) * scale - 1, (y + 1) * scale - 1],
                    fill=palette[key]
                )
    return img

def save(img, path):
    img.save(path)
    print(f"Generated: {path}")

# ==================== 配色 ====================
PALETTE = {
    '.': (0, 0, 0, 0),        # 透明
    'K': (15, 15, 26, 255),   # 深色背景
    'W': (226, 232, 240, 255), # 白色文字
    'G1': (74, 222, 128, 255), # 草元素亮
    'G2': (34, 197, 94, 255),  # 草元素暗
    'B1': (59, 130, 246, 255), # 水元素亮
    'B2': (37, 99, 235, 255),  # 水元素暗
    'R1': (239, 68, 68, 255),  # 火元素亮
    'R2': (220, 38, 38, 255),  # 火元素暗
    'P1': (168, 85, 247, 255), # 雷元素亮
    'P2': (147, 51, 234, 255), # 雷元素暗
    'C1': (103, 232, 249, 255), # 冰元素亮
    'C2': (34, 211, 238, 255),  # 冰元素暗
    'Y1': (251, 191, 36, 255),  # 岩/金亮
    'Y2': (245, 158, 11, 255),  # 岩/金暗
    'O1': (251, 146, 60, 255),  # 橙亮
    'O2': (234, 88, 12, 255),   # 橙暗
    'V1': (124, 58, 237, 255),  # 紫主色亮
    'V2': (91, 33, 182, 255),   # 紫主色暗
    'N1': (148, 163, 184, 255), # 中性灰亮
    'N2': (100, 116, 139, 255), # 中性灰暗
    'N3': (71, 85, 105, 255),   # 深灰
    'D': (30, 30, 50, 255),     # 深色
}

def glow_layer(base_grid, glow_color, radius=1):
    """为像素网格添加发光效果"""
    h = len(base_grid)
    w = len(base_grid[0]) if h > 0 else 0
    result = [['.' for _ in range(w)] for _ in range(h)]
    for y in range(h):
        for x in range(w):
            if base_grid[y][x] != '.':
                result[y][x] = base_grid[y][x]
            else:
                # Check neighbors
                has_neighbor = False
                for dy in range(-radius, radius+1):
                    for dx in range(-radius, radius+1):
                        ny, nx = y + dy, x + dx
                        if 0 <= ny < h and 0 <= nx < w and base_grid[ny][nx] != '.':
                            has_neighbor = True
                            break
                    if has_neighbor:
                        break
                if has_neighbor:
                    result[y][x] = glow_color
    return result

# ==================== 1. 应用图标 ====================
def gen_app_icon():
    # 16x16 pixel grid scaled to 256x256
    grid = [
        "................",
        "......KKKK......",
        "....KKVVVVKK....",
        "...KVV111VVK...",
        "..KV1111111VK..",
        ".KV111WWW111VK.",
        ".KV11WWWWW11VK.",
        "KV111WWWWW111VK",
        "KV1111WWW1111VK",
        "KV11111W11111VK",
        ".KV111111111VK.",
        ".KV111111111VK.",
        "..KV1111111VK..",
        "...KVV111VVK...",
        "....KKVVVVKK....",
        "......KKKK......",
    ]
    # V=紫水晶, 1=高光, W=文字颜色
    p = {**PALETTE, '1': (167, 139, 250, 255)}
    img = create_pixel_image(grid, p, scale=32)
    save(img, os.path.join(OUTPUT_DIR, "ui", "app_icon_512.png"))
    
    # Also 256 version
    img256 = img.resize((256, 256), Image.NEAREST)
    save(img256, os.path.join(OUTPUT_DIR, "ui", "app_icon_256.png"))
    
    # 32x32 for tray
    img32 = img.resize((32, 32), Image.NEAREST)
    save(img32, os.path.join(OUTPUT_DIR, "ui", "app_icon_32.png"))

# ==================== 2. 传送门 ====================
def gen_portals():
    # 晨曦之门 - 橙色太阳主题
    morning = [
        "................",
        ".....OOOOOO.....",
        "...OOYYYYYYOO...",
        "..OYY......YYO..",
        ".OY....YY....YO.",
        ".OY...YYYY...YO.",
        "OY...YYYYYY...YO",
        "OY...YYYYYY...YO",
        "OY...YYYYYY...YO",
        "OY...YYYYYY...YO",
        ".OY...YYYY...YO.",
        ".OY....YY....YO.",
        "..OYY......YYO..",
        "...OOYYYYYYOO...",
        ".....OOOOOO.....",
        "................",
    ]
    p_morning = {**PALETTE, 'O': PALETTE['O2'], 'Y': PALETTE['Y1']}
    img = create_pixel_image(morning, p_morning, scale=16)
    save(img, os.path.join(OUTPUT_DIR, "portals", "portal_morning.png"))
    
    # 烈日之门 - 红色火焰主题
    noon = [
        "................",
        "......RRRR......",
        "....RRR11RRR....",
        "...RR1....1RR...",
        "..R1...RR...1R..",
        ".R1...R11R...1R.",
        ".R...R1111R...R.",
        "R...R111111R...R",
        "R...R111111R...R",
        ".R...R1111R...R.",
        ".R1...R11R...1R.",
        "..R1...RR...1R..",
        "...RR1....1RR...",
        "....RRR11RRR....",
        "......RRRR......",
        "................",
    ]
    p_noon = {**PALETTE, 'R': PALETTE['R2'], '1': PALETTE['R1']}
    img = create_pixel_image(noon, p_noon, scale=16)
    save(img, os.path.join(OUTPUT_DIR, "portals", "portal_noon.png"))
    
    # 星夜之门 - 紫色星空主题
    evening = [
        "................",
        "......VVVV......",
        "....VVV11VVV....",
        "...VV1....1VV...",
        "..V1...VV...1V..",
        ".V1...V11V...1V.",
        ".V...V1111V...V.",
        "V...V111111V...V",
        "V...V111111V...V",
        ".V...V1111V...V.",
        ".V1...V11V...1V.",
        "..V1...VV...1V..",
        "...VV1....1VV...",
        "....VVV11VVV....",
        "......VVVV......",
        "................",
    ]
    p_evening = {**PALETTE, 'V': PALETTE['V2'], '1': PALETTE['V1']}
    img = create_pixel_image(evening, p_evening, scale=16)
    save(img, os.path.join(OUTPUT_DIR, "portals", "portal_evening.png"))

# ==================== 3. 元素水晶 ====================
def gen_crystals():
    # 基础水晶形状 (16x16)
    base_crystal = [
        "................",
        ".......11.......",
        "......1111......",
        ".....111111.....",
        "....11111111....",
        "...1111111111...",
        "..111111111111..",
        ".11111111111111.",
        ".11111111111111.",
        "..111111111111..",
        "...1111111111...",
        "....11111111....",
        ".....111111.....",
        "......1111......",
        ".......11.......",
        "................",
    ]
    
    elements = [
        ('grass', 'G', 'G1', 'G2'),
        ('water', 'B', 'B1', 'B2'),
        ('fire', 'R', 'R1', 'R2'),
        ('thunder', 'P', 'P1', 'P2'),
        ('ice', 'C', 'C1', 'C2'),
        ('rock', 'Y', 'Y1', 'Y2'),
    ]
    
    for name, key, light, dark in elements:
        # 明亮版本
        p = {**PALETTE, '1': PALETTE[light], '.': (0,0,0,0)}
        img = create_pixel_image(base_crystal, p, scale=8)
        save(img, os.path.join(OUTPUT_DIR, "crystals", f"crystal_{name}_bright.png"))
        
        # 暗淡版本 (灰暗水晶)
        p_dim = {**PALETTE, '1': PALETTE['N3'], '.': (0,0,0,0)}
        img_dim = create_pixel_image(base_crystal, p_dim, scale=8)
        save(img_dim, os.path.join(OUTPUT_DIR, "crystals", f"crystal_{name}_dim.png"))
        
        # 微光版本
        p_faint = {**PALETTE, '1': PALETTE['N2'], '.': (0,0,0,0)}
        img_faint = create_pixel_image(base_crystal, p_faint, scale=8)
        save(img_faint, os.path.join(OUTPUT_DIR, "crystals", f"crystal_{name}_faint.png"))

# ==================== 4. 像素方块 ====================
def gen_blocks():
    # 普通方块
    normal = [
        "KKKKKKKK",
        "KN1N1N1K",
        "KN1N1N1K",
        "KN1N1N1K",
        "KN1N1N1K",
        "KN1N1N1K",
        "KN1N1N1K",
        "KKKKKKKK",
    ]
    p = {**PALETTE, 'N1': PALETTE['N2']}
    img = create_pixel_image(normal, p, scale=8)
    save(img, os.path.join(OUTPUT_DIR, "blocks", "block_normal.png"))
    
    # 稀有方块 (发光纹理)
    rare = [
        "VVVVVVVV",
        "V1V1V1V1",
        "V1V1V1V1",
        "V1V1V1V1",
        "V1V1V1V1",
        "V1V1V1V1",
        "V1V1V1V1",
        "VVVVVVVV",
    ]
    p_rare = {**PALETTE, '1': PALETTE['V1']}
    img = create_pixel_image(rare, p_rare, scale=8)
    save(img, os.path.join(OUTPUT_DIR, "blocks", "block_rare.png"))
    
    # 限定方块 (彩虹)
    special = [
        "RRRRRRRR",
        "RGGGGGGR",
        "RGBBBBGR",
        "RGBYYBGR",
        "RGBYYBGR",
        "RGBBBBGR",
        "RGGGGGGR",
        "RRRRRRRR",
    ]
    p_special = {
        **PALETTE,
        'R': PALETTE['R1'],
        'G': PALETTE['G1'],
        'B': PALETTE['B1'],
        'Y': PALETTE['Y1'],
    }
    img = create_pixel_image(special, p_special, scale=8)
    save(img, os.path.join(OUTPUT_DIR, "blocks", "block_special.png"))

# ==================== 5. 宝箱 ====================
def gen_chests():
    # 小宝箱
    small = [
        "................",
        "................",
        "...YYYYYYYYYY...",
        "..YYYYYYYYYYYY..",
        ".YYYYYYYYYYYYYY.",
        ".YYYYYYWWYYYYYY.",
        "YYYYYYYWWYYYYYYY",
        "YYYYYYYYYYYYYYYY",
        "YYYYYYYYYYYYYYYY",
        "Y1YYYYYYYYYYYY1Y",
        "Y1YYYYYYYYYYYY1Y",
        "YYYYYYYYYYYYYYYY",
        ".YYYYYYYYYYYYYY.",
        ".YYYYYYYYYYYYYY.",
        "..YYYYYYYYYYYY..",
        "...YYYYYYYYYY...",
    ]
    p = {**PALETTE, '1': PALETTE['Y2']}
    img = create_pixel_image(small, p, scale=8)
    save(img, os.path.join(OUTPUT_DIR, "ui", "chest_small.png"))
    
    # 大宝箱
    large = [
        "................",
        "...VVVVVVVVVV...",
        "..VVVVVVVVVVVV..",
        ".VVVVVVVVVVVVVV.",
        ".VVVVVVVVVVVVVV.",
        "VVVVVVWWWWVVVVVV",
        "VVVVVVWWWWVVVVVV",
        "VVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVV",
        "V1VVVVVVVVVVVV1V",
        "V1VVVVVVVVVVVV1V",
        "VVVVVVVVVVVVVVVV",
        ".VVVVVVVVVVVVVV.",
        ".VVVVVVVVVVVVVV.",
        "..VVVVVVVVVVVV..",
    ]
    p_large = {**PALETTE, '1': PALETTE['V1']}
    img = create_pixel_image(large, p_large, scale=8)
    save(img, os.path.join(OUTPUT_DIR, "ui", "chest_large.png"))

# ==================== 6. 魔王 ====================
def gen_boss():
    # 像素魔王 (16x16)
    boss = [
        "................",
        "......RRRR......",
        ".....RRRRRR.....",
        "....RRR11RRR....",
        "...R1.RRRR.1R...",
        "...R1.RRRR.1R...",
        "..RRRRRRRRRRRR..",
        "..R1RRRRRRRR1R..",
        ".R1.RRRRRRRR.1R.",
        ".R1.RRRRRRRR.1R.",
        ".RR.RR1RR1RR.RR.",
        "...RR1RRRR1RR...",
        "...RRRRRRRRRR...",
        "....RRRRRRRR....",
        ".....RR..RR.....",
        "......RR..RR....",
    ]
    p_boss = {**PALETTE, '1': PALETTE['R1']}
    img = create_pixel_image(boss, p_boss, scale=12)
    save(img, os.path.join(OUTPUT_DIR, "ui", "boss.png"))

# ==================== 7. 徽章 ====================
def gen_badges():
    # 徽章基础形状 - 盾牌
    shield = [
        ".......11.......",
        "......1111......",
        ".....111111.....",
        "....11111111....",
        "...1111111111...",
        "..111111111111..",
        ".11111111111111.",
        ".11111111111111.",
        ".11111111111111.",
        "..111111111111..",
        "...1111111111...",
        "....11111111....",
        ".....111111.....",
        "......1111......",
        ".......11.......",
        "................",
    ]
    
    badges = [
        ('sprout', 'G1', '🌱'),    # 初出茅庐
        ('fire', 'O1', '🔥'),       # 七日之火
        ('sword', 'N1', '⚔️'),      # 魔王克星
        ('builder', 'Y1', '🏠'),    # 建造师
        ('collector', 'V1', '💎'),  # 水晶收藏家
        ('perfect', 'R1', '🎯'),    # 百发百中
        ('night', 'B1', '🌙'),      # 夜行者
    ]
    
    for name, color_key, _emoji in badges:
        p = {**PALETTE, '1': PALETTE[color_key]}
        img = create_pixel_image(shield, p, scale=8)
        save(img, os.path.join(OUTPUT_DIR, "badges", f"badge_{name}.png"))

# ==================== 8. 效果/粒子 ====================
def gen_effects():
    # XP 星星
    star = [
        ".......Y.......",
        ".......Y.......",
        ".......Y.......",
        "...Y...Y...Y...",
        "....Y.Y.Y.Y....",
        ".....YYYYY.....",
        "YYYYYYYYYYYYYYY",
        ".....YYYYY.....",
        "....Y.Y.Y.Y....",
        "...Y...Y...Y...",
        ".......Y.......",
        ".......Y.......",
        ".......Y.......",
        "...............",
        "...............",
    ]
    p = {**PALETTE, 'Y': PALETTE['Y1']}
    img = create_pixel_image(star, p, scale=6)
    save(img, os.path.join(OUTPUT_DIR, "effects", "star.png"))
    
    # 闪光
    sparkle = [
        ".......1.......",
        ".......1.......",
        ".......1.......",
        "...1...1...1...",
        "....1..1..1....",
        ".....1.1.1.....",
        "111111111111111",
        ".....1.1.1.....",
        "....1..1..1....",
        "...1...1...1...",
        ".......1.......",
        ".......1.......",
        ".......1.......",
        "...............",
        "...............",
    ]
    p_spark = {**PALETTE, '1': PALETTE['W']}
    img = create_pixel_image(sparkle, p_spark, scale=6)
    save(img, os.path.join(OUTPUT_DIR, "effects", "sparkle.png"))

# ==================== 9. 区域地图预览 ====================
def gen_zone_previews():
    zones = [
        ('newbie', 'N1', 'N2'),   # 新手村 - 灰
        ('grass', 'G1', 'G2'),    # 清风平原 - 绿
        ('water', 'B1', 'B2'),    # 蓝水湖泊 - 蓝
        ('fire', 'R1', 'R2'),     # 赤焰山脉 - 红
        ('thunder', 'P1', 'P2'),  # 雷霆峡谷 - 紫
        ('ice', 'C1', 'C2'),      # 永冬之巅 - 青
    ]
    
    for name, light, dark in zones:
        # 简单的地形预览 (32x32)
        grid = []
        for y in range(32):
            row = ""
            for x in range(32):
                if (x + y) % 7 == 0:
                    row += '2'
                elif (x * 3 + y * 2) % 5 == 0:
                    row += '1'
                else:
                    row += '.'
            grid.append(row)
        
        p = {
            **PALETTE,
            '1': PALETTE[light],
            '2': PALETTE[dark],
            '.': (15, 15, 26, 255),
        }
        img = create_pixel_image(grid, p, scale=4)
        save(img, os.path.join(OUTPUT_DIR, "ui", f"zone_{name}.png"))

if __name__ == "__main__":
    ensure_dirs()
    gen_app_icon()
    gen_portals()
    gen_crystals()
    gen_blocks()
    gen_chests()
    gen_boss()
    gen_badges()
    gen_effects()
    gen_zone_previews()
    print("\n✅ All assets generated successfully!")
