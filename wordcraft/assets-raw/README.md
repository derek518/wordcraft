# 生成原图

图像模型输出的 1024×1024 原图，压制前的样子。

## 为什么入库

`.gitignore` 排除了 `src-tauri/audio/`，因为那 38MB 音频由
`scripts/tts/pregenerate.py` 确定性产出，删了随时能重跑。

**这些不行。** 图像模型有随机性——同样的提示词、同样的参数，出来的也是另一张图。
删掉就是永久丢失，而压制参数很可能还要再调：

- 卡面第一版压到 50×50 锁 8 色，12 张角色卡全毁，靠原图重压才救回来
- 界面精灵图第一版压到 16 格，九张全毁，同样靠原图重压救回

两次都是压制的问题，两次都靠原图挽回。这就是它们留在这里的理由。

> 42 张卡面的原图**已经丢了**——重压之后被当成中间产物清掉了。若日后还要
> 改卡面的压制参数，只能整批重新生成，风格未必对得上。这个目录是为了不再犯同样的错。

## 重压

```bash
python3 scripts/cards/conform.py assets-raw/ui -g 48 -o public/assets/ui   # 魔王四档
python3 scripts/cards/conform.py assets-raw/ui -g 32 -o public/assets/ui   # 赛车 · 奖牌
```

网格按题材复杂度选，不是按显示尺寸选。详见 `docs/card-art-prompts.md` §2 与 §11。
