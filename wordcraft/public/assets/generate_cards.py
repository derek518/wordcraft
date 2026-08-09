#!/usr/bin/env python3
"""WordCraft Card Asset Generator v2 - 精致像素卡牌"""
import os, math
from PIL import Image, ImageDraw

OUTPUT_DIR = os.path.dirname(os.path.abspath(__file__))

def ensure_dirs():
    for subdir in ["cards/back", "cards/common", "cards/rare", "cards/legend"]:
        os.makedirs(os.path.join(OUTPUT_DIR, subdir), exist_ok=True)

def save(img, path):
    img.save(path)
    print(f"Generated: {path}")

# 配色
ELEMENTS = {
    'grass':  {'light': (74, 222, 128), 'mid': (34, 197, 94),  'dark': (21, 128, 61)},
    'water':  {'light': (96, 165, 250), 'mid': (59, 130, 246), 'dark': (29, 78, 216)},
    'fire':   {'light': (251, 113, 133),'mid': (239, 68, 68),  'dark': (185, 28, 28)},
    'thunder':{'light': (192, 132, 252),'mid': (168, 85, 247), 'dark': (126, 34, 206)},
    'ice':    {'light': (103, 232, 249),'mid': (34, 211, 238), 'dark': (8, 145, 178)},
    'rock':   {'light': (251, 191, 36), 'mid': (245, 158, 11), 'dark': (180, 83, 9)},
}
RARITY_BORDER = {1: (100, 116, 139), 2: (59, 130, 246), 3: (251, 191, 36)}

def bg_grad(draw, h, dark):
    for y in range(h):
        r = int(10 + (dark[0]-10) * (y/h) * 0.4)
        g = int(14 + (dark[1]-14) * (y/h) * 0.4)
        b = int(26 + (dark[2]-26) * (y/h) * 0.4)
        draw.line([(0,y),(200,y)], fill=(r,g,b,255))

def glow(draw, cx, cy, r, color, n=5):
    for i in range(n, 0, -1):
        a = int(30*(n-i+1)/n)
        draw.ellipse([cx-r-i*3, cy-r-i*3, cx+r+i*3, cy+r+i*3], fill=(*color[:3], a))

def star(draw, cx, cy, r, pts=5, fill=None, outline=None):
    p = []
    for i in range(pts*2):
        a = math.pi/pts*i - math.pi/2
        rad = r if i%2==0 else r/2
        p.append((cx+rad*math.cos(a), cy+rad*math.sin(a)))
    draw.polygon(p, fill=fill, outline=outline)

def rrect(draw, xy, radius, fill=None, outline=None, width=1):
    draw.rounded_rectangle(xy, radius=radius, fill=fill, outline=outline, width=width)

# ====== 卡背 ======
def gen_backs():
    for rid, name in [(1,'common'),(2,'rare'),(3,'legend')]:
        bc = RARITY_BORDER[rid]
        img = Image.new("RGBA", (200,280), (15,23,42,255))
        d = ImageDraw.Draw(img)
        rrect(d, [4,4,196,276], 12, outline=bc, width=3)
        rrect(d, [12,12,188,268], 8, outline=(*bc[:3],100), width=1)
        cx, cy = 100, 140
        if rid == 1:
            d.ellipse([cx-28, cy-28, cx+28, cy+28], outline=(*bc[:3],80), width=2)
        elif rid == 2:
            d.polygon([(cx, cy-32), (cx+22, cy), (cx, cy+32), (cx-22, cy)], fill=(*bc[:3],60), outline=bc, width=2)
            d.polygon([(cx, cy-18), (cx+13, cy), (cx, cy+18), (cx-13, cy)], fill=(*bc[:3],120))
        else:
            star(d, cx, cy, 38, pts=6, outline=bc)
            star(d, cx, cy, 24, pts=6, fill=(*bc[:3],80))
        for dx, dy in [(-1,-1),(1,-1),(-1,1),(1,1)]:
            d.ellipse([cx+dx*68-3, cy+dy*88-3, cx+dx*68+3, cy+dy*88+3], fill=(*bc[:3],60))
        save(img, os.path.join(OUTPUT_DIR, "cards", "back", f"back_{name}.png"))

# ====== 普通卡 ======
COMMON = {
    'grass':  [('leaf_shard','shard'),('sprout','creature'),('vine_bug','creature'),('herb_pouch','item')],
    'water':  [('water_drop','shard'),('bubble_fish','creature'),('jelly_baby','creature'),('conch_shell','item')],
    'fire':   [('ember_shard','shard'),('small_flame','creature'),('lava_worm','creature'),('torch','item')],
    'thunder':[('spark_shard','shard'),('static_bug','creature'),('cloud_baby','creature'),('battery','item')],
    'ice':    [('ice_shard','shard'),('snow_ball','creature'),('frost_bug','creature'),('ice_spike','item')],
    'rock':   [('sand_shard','shard'),('rock_beetle','creature'),('miner_rat','creature'),('pickaxe','item')],
}

def draw_shard(d, cx, cy, c, s=20):
    d.polygon([(cx, cy-s),(cx+s*0.6, cy-s*0.3),(cx+s*0.4, cy+s*0.5),(cx-s*0.4, cy+s*0.5),(cx-s*0.6, cy-s*0.3)], fill=c['mid'], outline=c['light'])
    d.polygon([(cx, cy-s*0.7),(cx+s*0.3, cy-s*0.2),(cx, cy+s*0.1)], fill=(*c['light'][:3],150))

def draw_creature(d, cx, cy, c, v=0):
    if v == 0:
        d.ellipse([cx-16, cy-13, cx+16, cy+13], fill=c['mid'], outline=c['dark'])
        d.ellipse([cx-5, cy-7, cx+2, cy+1], fill=(255,255,255))
        d.ellipse([cx-3, cy-5, cx, cy-1], fill=(0,0,0))
    elif v == 1:
        d.ellipse([cx-10, cy-16, cx+10, cy+16], fill=c['mid'], outline=c['dark'])
        d.ellipse([cx-3, cy-9, cx+3, cy-3], fill=(255,255,255))
        d.ellipse([cx-1, cy-7, cx+2, cy-4], fill=(0,0,0))
    else:
        d.rounded_rectangle([cx-13, cy-10, cx+13, cy+10], radius=5, fill=c['mid'], outline=c['dark'])
        d.ellipse([cx-7, cy-5, cx, cy+2], fill=(255,255,255))
        d.ellipse([cx-5, cy-3, cx-1, cy], fill=(0,0,0))

def draw_item(d, cx, cy, c, t):
    if t == 'herb_pouch':
        d.rounded_rectangle([cx-10, cy-8, cx+10, cy+8], radius=3, fill=(139,90,43), outline=(100,60,30))
        d.ellipse([cx-3, cy-5, cx+3, cy-1], fill=c['light'])
    elif t == 'conch_shell':
        d.polygon([(cx, cy-12), (cx+8, cy+4), (cx-4, cy+8)], fill=(210,180,140), outline=(180,150,110))
    elif t == 'torch':
        d.rectangle([cx-2, cy-4, cx+2, cy+12], fill=(139,90,43))
        d.ellipse([cx-6, cy-12, cx+6, cy-4], fill=c['light'], outline=c['mid'])
    elif t == 'battery':
        d.rounded_rectangle([cx-7, cy-10, cx+7, cy+10], radius=2, fill=(100,116,139), outline=(71,85,105))
        d.rectangle([cx-2, cy-14, cx+2, cy-10], fill=(148,163,184))
        d.ellipse([cx-1, cy-5, cx+1, cy-1], fill=c['light'])
    elif t == 'ice_spike':
        d.polygon([(cx, cy-15), (cx+7, cy+8), (cx-7, cy+8)], fill=c['light'], outline=c['mid'])
    elif t == 'pickaxe':
        d.rectangle([cx-1, cy-12, cx+1, cy+8], fill=(139,90,43))
        d.polygon([(cx-10, cy-12), (cx, cy-8), (cx+10, cy-12)], fill=(148,163,184), outline=(100,116,139))

def gen_common():
    for elem, cards in COMMON.items():
        c = ELEMENTS[elem]
        for cid, ctype in cards:
            img = Image.new("RGBA", (200,280))
            d = ImageDraw.Draw(img)
            bg_grad(d, 280, c['dark'])
            bc = RARITY_BORDER[1]
            rrect(d, [4,4,196,276], 12, outline=bc, width=2)
            rrect(d, [10,10,190,270], 8, outline=(*c['mid'][:3],100), width=1)
            cx, cy = 100, 115
            glow(d, cx, cy, 48, c['light'], 4)
            if ctype == 'shard': draw_shard(d, cx, cy, c, 20)
            elif ctype == 'creature': draw_creature(d, cx, cy, c, hash(cid) % 3)
            else: draw_item(d, cx, cy, c, cid)
            d.rounded_rectangle([20, 230, 180, 265], radius=6, fill=(15,23,42,200))
            d.ellipse([12, 12, 22, 22], fill=bc)
            save(img, os.path.join(OUTPUT_DIR, "cards", "common", f"{elem}_{cid}.png"))

# ====== 稀有卡 ======
RARE = {
    'grass':  [('thorn_guard','guardian'),('life_seed','artifact')],
    'water':  [('tide_herald','guardian'),('deep_pearl','artifact')],
    'fire':   [('flame_knight','guardian'),('magma_heart','artifact')],
    'thunder':[('storm_eye','guardian'),('thunder_hammer','artifact')],
    'ice':    [('frost_warden','guardian'),('eternal_mirror','artifact')],
    'rock':   [('ridge_giant','guardian'),('gold_shield','artifact')],
}

def draw_guardian(d, cx, cy, c):
    d.ellipse([cx-11, cy-28, cx+11, cy-10], fill=c['mid'], outline=c['light'], width=2)
    d.polygon([(cx, cy-7), (cx+16, cy+18), (cx-16, cy+18)], fill=c['dark'], outline=c['mid'])
    d.rectangle([cx+18, cy-22, cx+22, cy+22], fill=(139,90,43))
    d.ellipse([cx+14, cy-28, cx+26, cy-18], fill=c['light'], outline=c['mid'])
    d.ellipse([cx-5, cy-21, cx-1, cy-17], fill=(255,200,50))
    d.ellipse([cx+1, cy-21, cx+5, cy-17], fill=(255,200,50))

def draw_artifact(d, cx, cy, c, t):
    if t == 'life_seed':
        d.ellipse([cx-9, cy-7, cx+9, cy+7], fill=c['light'], outline=c['mid'])
        d.ellipse([cx-2, cy-2, cx+2, cy+2], fill=(255,255,200))
    elif t == 'deep_pearl':
        d.ellipse([cx-11, cy-11, cx+11, cy+11], fill=(230,240,255), outline=(180,200,230))
        d.ellipse([cx-3, cy-5, cx+1, cy], fill=(255,255,255))
    elif t == 'magma_heart':
        d.polygon([(cx, cy+7), (cx-11, cy-4), (cx-5, cy-11), (cx, cy-7), (cx+5, cy-11), (cx+11, cy-4)], fill=c['light'], outline=c['mid'])
    elif t == 'thunder_hammer':
        d.rectangle([cx-2, cy-18, cx+2, cy+13], fill=(148,163,184))
        d.rectangle([cx-11, cy-23, cx+11, cy-13], fill=c['light'], outline=c['mid'])
    elif t == 'eternal_mirror':
        d.rounded_rectangle([cx-13, cy-16, cx+13, cy+16], radius=3, fill=(200,220,240), outline=c['mid'])
        d.ellipse([cx-7, cy-7, cx+7, cy+7], fill=(150,180,210))
    else:
        d.polygon([(cx, cy-18), (cx+16, cy-4), (cx+11, cy+16), (cx, cy+20), (cx-11, cy+16), (cx-16, cy-4)], fill=c['mid'], outline=c['light'])

def gen_rare():
    for elem, cards in RARE.items():
        c = ELEMENTS[elem]
        for cid, ctype in cards:
            img = Image.new("RGBA", (200,280))
            d = ImageDraw.Draw(img)
            bg_grad(d, 280, c['dark'])
            bc = RARITY_BORDER[2]
            rrect(d, [3,3,197,277], 12, outline=bc, width=3)
            rrect(d, [9,9,191,271], 8, outline=(*bc[:3],80), width=2)
            for dx, dy in [(-1,-1),(1,-1),(-1,1),(1,1)]:
                star(d, 100+dx*83, 140+dy*118, 5, pts=4, fill=(*bc[:3],60))
            cx, cy = 100, 115
            glow(d, cx, cy, 52, c['light'], 5)
            if ctype == 'guardian': draw_guardian(d, cx, cy, c)
            else: draw_artifact(d, cx, cy, c, cid)
            d.rounded_rectangle([15, 225, 185, 265], radius=8, fill=(15,23,42,220))
            d.polygon([(20,16),(25,21),(20,26),(15,21)], fill=bc)
            d.polygon([(28,16),(33,21),(28,26),(23,21)], fill=(*bc[:3],150))
            save(img, os.path.join(OUTPUT_DIR, "cards", "rare", f"{elem}_{cid}.png"))

# ====== 传说卡（精致版） ======
def draw_legend_creature(d, cx, cy, elem, colors):
    light, mid, dark = colors
    if elem == 'grass':
        d.ellipse([cx-14, cy-33, cx+14, cy-10], fill=mid, outline=light, width=2)
        d.polygon([(cx-9, cy-28), (cx-14, cy-48), (cx-4, cy-30)], fill=dark, outline=light)
        d.polygon([(cx+9, cy-28), (cx+14, cy-48), (cx+4, cy-30)], fill=dark, outline=light)
        d.ellipse([cx-18, cy-13, cx+18, cy+23], fill=(*mid[:3],200), outline=light, width=2)
        d.polygon([(cx-18, cy-5), (cx-42, cy-23), (cx-32, cy+8)], fill=(*light[:3],100), outline=light)
        d.polygon([(cx+18, cy-5), (cx+42, cy-23), (cx+32, cy+8)], fill=(*light[:3],100), outline=light)
        d.ellipse([cx-7, cy-21, cx-1, cy-15], fill=(255,220,100))
        d.ellipse([cx+1, cy-21, cx+7, cy-15], fill=(255,220,100))
        d.polygon([(cx-4, cy+20), (cx, cy+43), (cx+4, cy+20)], fill=dark, outline=light)
    elif elem == 'water':
        d.ellipse([cx-28, cy-18, cx+28, cy+18], fill=mid, outline=light, width=2)
        d.polygon([(cx-28, cy), (cx-47, cy-13), (cx-47, cy+13)], fill=dark, outline=light)
        d.polygon([(cx-9, cy+4), (cx-23, cy+23), (cx, cy+13)], fill=(*light[:3],120), outline=light)
        d.polygon([(cx+9, cy+4), (cx+23, cy+23), (cx, cy+13)], fill=(*light[:3],120), outline=light)
        d.ellipse([cx+18, cy-28, cx+26, cy-18], fill=(*light[:3],150))
        d.ellipse([cx+20, cy-36, cx+28, cy-28], fill=(*light[:3],100))
        d.ellipse([cx+13, cy-4, cx+19, cy+2], fill=(255,255,255))
        d.ellipse([cx+14, cy-3, cx+17, cy], fill=(0,0,0))
    elif elem == 'fire':
        d.ellipse([cx-11, cy-13, cx+11, cy+13], fill=mid, outline=light, width=2)
        d.ellipse([cx-7, cy-26, cx+7, cy-11], fill=(*light[:3],200), outline=light)
        d.polygon([(cx, cy-33), (cx-4, cy-43), (cx+4, cy-43)], fill=dark, outline=light)
        for side in [-1, 1]:
            pts = [(cx+side*11, cy-4), (cx+side*37, cy-23), (cx+side*32, cy+4), (cx+side*41, cy+8), (cx+side*27, cy+18), (cx+side*11, cy+8)]
            d.polygon(pts, fill=(*light[:3],80), outline=light)
        for i, off in enumerate([-13, 0, 13]):
            a = 140 - i*30
            d.polygon([(cx+off*0.5, cy+11), (cx+off, cy+37), (cx+off*0.5+4, cy+11)], fill=(*light[:3],a), outline=light)
        d.ellipse([cx-2, cy-21, cx+2, cy-17], fill=(255,200,50))
    elif elem == 'thunder':
        d.ellipse([cx-9, cy-9, cx+9, cy+18], fill=mid, outline=light, width=2)
        d.ellipse([cx-7, cy-24, cx+7, cy-8], fill=(*light[:3],180), outline=light)
        d.polygon([(cx, cy-19), (cx+5, cy-15), (cx, cy-11)], fill=(251,191,36))
        for side in [-1, 1]:
            pts = [(cx+side*9, cy-4), (cx+side*42, cy-18), (cx+side*46, cy+4), (cx+side*37, cy+13), (cx+side*9, cy+6)]
            d.polygon(pts, fill=(*mid[:3],150), outline=light)
        d.polygon([(cx-23, cy-33), (cx-18, cy-23), (cx-26, cy-23), (cx-20, cy-13)], fill=light)
        d.polygon([(cx+18, cy-28), (cx+26, cy-18), (cx+20, cy-18), (cx+28, cy-8)], fill=light)
        d.ellipse([cx-3, cy-17, cx+1, cy-13], fill=(255,200,50))
    elif elem == 'ice':
        d.ellipse([cx-13, cy-4, cx+13, cy+19], fill=mid, outline=light, width=2)
        d.polygon([(cx, cy-24), (cx-11, cy-10), (cx+11, cy-10)], fill=(*light[:3],200), outline=light)
        d.polygon([(cx-9, cy-19), (cx-14, cy-34), (cx-4, cy-21)], fill=dark, outline=light)
        d.polygon([(cx+9, cy-19), (cx+14, cy-34), (cx+4, cy-21)], fill=dark, outline=light)
        d.ellipse([cx+10, cy+4, cx+28, cy+14], fill=(*mid[:3],150), outline=light)
        for pos in [(cx-23, cy-19), (cx+23, cy-14), (cx, cy-38)]:
            d.polygon([(pos[0], pos[1]-7), (pos[0]+4, pos[1]), (pos[0], pos[1]+7), (pos[0]-4, pos[1])], fill=(*light[:3],120), outline=(255,255,255))
        d.ellipse([cx-4, cy-17, cx, cy-13], fill=(200,230,255))
        d.ellipse([cx+1, cy-17, cx+5, cy-13], fill=(200,230,255))
    else:
        shell_pts = []
        for i in range(6):
            a = math.pi/3*i
            shell_pts.append((cx+26*math.cos(a), cy+4+20*math.sin(a)))
        d.polygon(shell_pts, fill=dark, outline=light, width=2)
        import math as m
        hex_pts = []
        for i in range(6):
            a = m.pi/3*i - m.pi/6
            hex_pts.append((cx+11*m.cos(a), cy+4+11*m.sin(a)))
        d.polygon(hex_pts, fill=(*mid[:3],150), outline=light)
        d.ellipse([cx-9, cy-23, cx+9, cy-7], fill=mid, outline=light)
        for angle in [0.3, 0.8, 2.3, 2.8]:
            lx = cx + 30*m.cos(angle)
            ly = cy + 4 + 23*m.sin(angle)
            d.ellipse([lx-5, ly-7, lx+5, ly+7], fill=dark, outline=light)
        d.ellipse([cx-4, cy-16, cx, cy-12], fill=(255,220,100))
        d.ellipse([cx+1, cy-16, cx+5, cy-12], fill=(255,220,100))

def gen_legend():
    legends = [
        ('grass',  [(74, 222, 128), (34, 197, 94),  (21, 128, 61)]),
        ('water',  [(96, 165, 250), (59, 130, 246), (29, 78, 216)]),
        ('fire',   [(251, 113, 133),(239, 68, 68),  (185, 28, 28)]),
        ('thunder',[(192, 132, 252),(168, 85, 247), (126, 34, 206)]),
        ('ice',    [(103, 232, 249),(34, 211, 238), (8, 145, 178)]),
        ('rock',   [(251, 191, 36), (245, 158, 11), (180, 83, 9)]),
    ]
    gold = (251, 191, 36)
    for elem, colors in legends:
        light, mid, dark = colors
        img = Image.new("RGBA", (200,280))
        d = ImageDraw.Draw(img)
        for y in range(280):
            r = int(8 + (dark[0]-8)*(y/280)*0.15)
            g = int(8 + (dark[1]-8)*(y/280)*0.15)
            b = int(18 + (dark[2]-18)*(y/280)*0.15)
            d.line([(0,y),(200,y)], fill=(r,g,b,255))
        rrect(d, [2,2,198,278], 14, outline=(*gold[:3],60), width=4)
        rrect(d, [6,6,194,274], 10, outline=gold, width=2)
        rrect(d, [14,14,186,266], 6, outline=(*light[:3],100), width=1)
        cs = 16
        for dx, dy in [(-1,-1),(1,-1),(-1,1),(1,1)]:
            cx = 100 + dx*80; cy = 140 + dy*116
            d.polygon([(cx, cy-cs),(cx+cs*0.7, cy),(cx, cy+cs),(cx-cs*0.7, cy)], fill=None, outline=gold, width=2)
            star(d, cx, cy, 5, pts=4, fill=(*gold[:3],80))
        cx, cy = 100, 125
        for i in range(8, 0, -1):
            a = int(25*(9-i)/8)
            d.ellipse([cx-58-i*4, cy-58-i*4, cx+58+i*4, cy+58+i*4], fill=(*light[:3], a))
        draw_legend_creature(d, cx, cy, elem, colors)
        d.polygon([(30,228),(100,216),(170,228),(170,242),(100,230),(30,242)], fill=(*gold[:3],40), outline=(*gold[:3],80))
        star(d, 100, 24, 9, pts=5, fill=(*gold[:3],120), outline=gold)
        star(d, 100, 24, 4, pts=5, fill=gold)
        save(img, os.path.join(OUTPUT_DIR, "cards", "legend", f"{elem}_guardian.png"))

if __name__ == "__main__":
    ensure_dirs()
    gen_backs()
    gen_common()
    gen_rare()
    gen_legend()
    print("\n✅ All 45 card assets generated!")
