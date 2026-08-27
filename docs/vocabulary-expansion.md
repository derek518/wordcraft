# 扩充词库（四级及以后）

高中范围现有 2076 词。要再扩，走这条流水线。**这是一次数据任务，不是改代码**——
代码侧已经就绪：`extract.py` 认 `--include-cet4`，学习范围的选项由数据库现查，
四级词一进库，设置里就会多出「四级」这一档，不必再改前端。

## 为什么四级词单列一档，而不是并进高中

一个词既是高考词又是四级词时，它首先是高考词——`extract.py` 按 `zk → gk → cet4`
的优先级定级。只有考纲之外的词才落到 `cet4`。

分档是为了让用户自己决定：高考前把四级词混进来会稀释重点，而考完之后
它正好接着用。合在一起就没有这个选择权了。

## 步骤

```bash
# 1. 取源数据（约 100MB，不入库）
curl -L -o /tmp/ecdict.csv \
  https://raw.githubusercontent.com/skywind3000/ECDICT/master/ecdict.csv

# 2. 抽词
python3 scripts/wordlist/extract.py /tmp/ecdict.csv \
  --include-cet4 -o scripts/wordlist/words.json

# 3. 生成例句。需要 DEEPSEEK_API_KEY（放 .env，已被 .gitignore 排除）
#    只为新词生成，已有例句会复用
python3 scripts/wordlist/gen_examples.py

# 4. 合成词库
python3 scripts/wordlist/build_library.py

# 5. 补发音。只生成缺失的，已有的 3657 个不重跑
python3 scripts/tts/pregenerate.py --concurrency 8
```

## 之后必须做的两件事

**① 写一条导入迁移。** 词库进库走 `import_words`，它按 `word` 唯一键做
upsert——已有词更新、新词插入，用户的学习状态（`word_states`）不受影响。
新增词的 `id` 会往后排，不会打乱既有记录。

**② 重新跑一遍 CI。** 音频缓存的 key 是 `hashFiles('wordcraft/public/library.json')`，
词库一变缓存自动失效，CI 会重新生成全部发音——那一次构建会慢很多（约 20 分钟），
之后恢复到 5 分钟。

## 别忘了

- `SOURCES.md` 要补一条：四级词同样来自 ECDICT（MIT），例句由 deepseek 生成
- 扩充后先在设置里看一眼「预计走完」的周数。四级词约 1800 个，
  按周末两天、每场 6 新词算是 50 周——多半需要同时调高每场新词数
