//! 词库读写与导入校验。契约见 contracts-v1.md §8。

use crate::db::clock;
use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

/// 受控 level 词表，contracts §8。
///
/// `cet4` 为考纲外扩展：单列一档而非并进 senior，用户可自行选择是否学。
/// 这张表在三处出现过——抽词脚本、build_library、这里——四级词导入时
/// 三处都要放行，漏一处就是「词进不来且没有报错」
const VALID_LEVELS: [&str; 4] = ["junior", "senior", "cet4", "art"];
const VALID_ZONES: [&str; 7] = [
    "newbie", "grass", "water", "fire", "thunder", "ice", "rock",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Word {
    pub id: i64,
    pub word: String,
    pub phonetic: String,
    pub pos: String,
    pub meaning: String,
    /// 第二词性。多数词没有同样常用的第二用法，`None` 就是「没有」。
    ///
    /// 出题只用主词性——选项长度一致才不会把正确答案暴露成「唯一那个长的」。
    /// 这一栏在揭晓时补充展示（契约 §8）。
    #[serde(default)]
    pub pos_2: Option<String>,
    #[serde(default)]
    pub meaning_2: Option<String>,
    pub example_1: String,
    pub example_2: String,
    pub level: String,
    pub frequency_band: i64,
    pub zone: String,
}

/// 导入载荷。字段与 `Word` 分离——导入时无 id，且需经校验才可入库。
#[derive(Debug, Clone, Deserialize)]
pub struct WordImport {
    pub word: String,
    #[serde(default)]
    pub phonetic: String,
    pub pos: String,
    pub meaning: String,
    /// 第二词性。多数词没有同样常用的第二用法，`None` 就是「没有」。
    ///
    /// 出题只用主词性——选项长度一致才不会把正确答案暴露成「唯一那个长的」。
    /// 这一栏在揭晓时补充展示（契约 §8）。
    #[serde(default)]
    pub pos_2: Option<String>,
    #[serde(default)]
    pub meaning_2: Option<String>,
    pub example_1: String,
    #[serde(default)]
    pub example_2: String,
    pub level: String,
    pub frequency_band: i64,
    /// 全局词频排名，能力模型的难度轴。
    ///
    /// 可空：18 个连字符复合词两个语料库都未收录。不插补——
    /// 编一个排名会让能力模型把凭空捏造的难度当成证据。
    #[serde(default)]
    pub frequency_rank: Option<i64>,
    pub zone: String,
    #[serde(default)]
    pub source_edition: String,
}

#[derive(Debug, Serialize)]
pub struct ImportOutcome {
    pub inserted: i64,
    pub updated: i64,
    pub rejected: Vec<RejectedWord>,
}

#[derive(Debug, Serialize)]
pub struct RejectedWord {
    pub word: String,
    pub reason: String,
}

fn row_to_word(row: &Row) -> rusqlite::Result<Word> {
    Ok(Word {
        id: row.get("id")?,
        word: row.get("word")?,
        phonetic: row.get("phonetic")?,
        pos: row.get("pos")?,
        meaning: row.get("meaning")?,
        pos_2: row.get("pos_2")?,
        meaning_2: row.get("meaning_2")?,
        example_1: row.get("example_1")?,
        example_2: row.get("example_2")?,
        level: row.get("level")?,
        frequency_band: row.get("frequency_band")?,
        zone: row.get("zone")?,
    })
}

const SELECT_COLS: &str = "id, word, phonetic, pos, meaning, pos_2, meaning_2, \
                           example_1, example_2, level, frequency_band, zone";

pub fn find_by_id(conn: &Connection, id: i64) -> Result<Option<Word>, String> {
    conn.query_row(
        &format!("SELECT {SELECT_COLS} FROM words WHERE id = ?1"),
        [id],
        row_to_word,
    )
    .optional()
    .map_err(|e| format!("查询词条 {id} 失败: {e}"))
}

/// 学习范围内的词数。界面上的分母应当用它——高中范围下显示 /3657
/// 等于把两千个不打算教的初中词也算进目标，与「已点亮」虚高是同一类失真。
pub fn count_in_scope(conn: &Connection, scope_sql: &str) -> Result<i64, String> {
    conn.query_row(
        &format!("SELECT COUNT(*) FROM words w WHERE {scope_sql}"),
        [],
        |r| r.get(0),
    )
    .map_err(|e| format!("统计范围内词数失败: {e}"))
}

pub fn count(conn: &Connection) -> Result<i64, String> {
    conn.query_row("SELECT COUNT(*) FROM words", [], |r| r.get(0))
        .map_err(|e| format!("统计词条数失败: {e}"))
}

pub fn search(conn: &Connection, keyword: &str, limit: i64) -> Result<Vec<Word>, String> {
    let pattern = format!("%{keyword}%");
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {SELECT_COLS} FROM words
             WHERE word LIKE ?1 OR meaning LIKE ?1
             ORDER BY frequency_band, word LIMIT ?2"
        ))
        .map_err(|e| format!("准备搜索语句失败: {e}"))?;

    let rows = stmt
        .query_map(rusqlite::params![pattern, limit], row_to_word)
        .map_err(|e| format!("搜索词条失败: {e}"))?;

    rows.collect::<Result<_, _>>()
        .map_err(|e| format!("读取搜索结果失败: {e}"))
}

/// 干扰项候选池：同词性、排除自身。契约 §6。
///
/// 仅返回候选，具体挑选规则（编辑距离、频段匹配、子串包含检查）由前端
/// `src/core/distractors.ts` 负责——那是纯逻辑，放在能被充分单测的一侧。
/// 干扰项是否与正确释义冲突。
///
/// 互为子串也算冲突：正确答案是「在……之后」时，「之后」作为选项等于把答案
/// 拆成两半摆在面前。
fn conflicts_with(candidate: &str, correct: &str) -> bool {
    candidate == correct || candidate.contains(correct) || correct.contains(candidate)
}

/// 干扰项候选池，contracts §6。
///
/// **返回内容随题型翻转**：Lv.1 是「看英文选中文」，干扰项必须是释义；
/// Lv.2–4 是「看中文/听音/看例句选英文」，干扰项必须是单词。搞反了题目就
/// 变成「看中文选中文」，四个选项全是同类，题面完全失效。
///
/// 三级降级：同词性 → 同区域 → 全库。**降级是必需的而非兜底**——词性标注
/// 细到 `prep./conj.` 这种粒度时，整个词库里可能只有一个词属于该词性，
/// 排除自身后候选池为空，题目就只剩正确答案一个选项。
///
/// 各级内部的排序策略也随题型变化（决议 S11）：Lv.1 纯随机，**刻意不用编辑
/// 距离**——把 `adapt/adopt/adept` 摆在一起会让初学者建立混淆记忆；形近区分
/// 从 Lv.2 起才逐步引入。
pub fn distractor_pool(
    conn: &Connection,
    word_id: i64,
    question_level: i64,
    limit: i64,
) -> Result<Vec<String>, String> {
    use std::collections::HashSet;

    let (word, meaning, pos, zone, band): (String, String, String, String, i64) = conn
        .query_row(
            "SELECT word, meaning, pos, zone, frequency_band FROM words WHERE id = ?1",
            [word_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(|e| format!("查询词条 {word_id} 失败: {e}"))?;

    // Lv.1 选释义，其余选单词
    let answer_col = if question_level <= 1 { "meaning" } else { "word" };
    let correct = if question_level <= 1 { &meaning } else { &word };

    let mut pool: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(correct.clone());

    let fetch = limit * 6; // 多取一些，冲突过滤与相似度排序后仍够用

    // 一级：同词性，按题型对应的相似度排序
    let sql = level_ordered_sql(answer_col, question_level, "pos = ?1 AND id != ?2");
    collect_distractors(
        conn,
        &sql,
        &[&pos, &word_id, &fetch],
        correct,
        limit,
        &mut pool,
        &mut seen,
        question_level,
        &word,
        band,
    )?;

    // 二级：同区域。同一区域的词难度相近，比全库随机更合适
    if (pool.len() as i64) < limit {
        let sql = level_ordered_sql(answer_col, question_level, "zone = ?1 AND id != ?2");
        collect_distractors(
            conn,
            &sql,
            &[&zone, &word_id, &fetch],
            correct,
            limit,
            &mut pool,
            &mut seen,
            question_level,
            &word,
            band,
        )?;
    }

    // 三级：全库。宁可干扰项跨区域，也不能让题目只有一个选项
    if (pool.len() as i64) < limit {
        let sql = format!(
            "SELECT {answer_col}, word, frequency_band FROM words WHERE id != ?1 ORDER BY RANDOM() LIMIT ?2"
        );
        collect_distractors(
            conn,
            &sql,
            &[&word_id, &fetch],
            correct,
            limit,
            &mut pool,
            &mut seen,
            question_level,
            &word,
            band,
        )?;
    }

    Ok(pool)
}

/// 按题型拼接候选查询。
///
/// Lv.4 例句挖空要求同频段——挖空处的选项若难度悬殊，用排除法就能猜中，
/// 考不出对目标词的掌握。SQL 层先按频段接近度排序，取到的候选更贴题。
fn level_ordered_sql(answer_col: &str, question_level: i64, where_clause: &str) -> String {
    let order = if question_level >= 4 {
        "ABS(frequency_band - (SELECT frequency_band FROM words WHERE id = ?2)), RANDOM()"
    } else {
        "RANDOM()"
    };
    format!("SELECT {answer_col}, word, frequency_band FROM words WHERE {where_clause} ORDER BY {order} LIMIT ?3")
}

/// Levenshtein 编辑距离。
///
/// 用于 Lv.2 优先挑选形近词。只需要小词长的比较，朴素 DP 足够。
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }

    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];

    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// 两词是否音近——近似判据：首字母相同且长度接近。
///
/// 契约要求「首音素相同」，严格实现需要音标解析（`/ˈkrɪstl/` 里剥离重音符号
/// 再切分音素）。首字母在英语里与首音素高度相关，且不会因音标格式差异而失效；
/// 加长度约束是为了排除 `a` 与 `abandon` 这类听感差异明显的配对。
fn sounds_similar(a: &str, b: &str) -> bool {
    let (fa, fb) = (a.chars().next(), b.chars().next());
    fa.is_some()
        && fa == fb
        && (a.len() as i64 - b.len() as i64).abs() <= 3
}

/// 一条候选：`answer` 是最终呈现给用户的选项文本（释义或单词），
/// 另两项仅用于相似度排序。
struct Candidate {
    answer: String,
    word: String,
    band: i64,
}

#[allow(clippy::too_many_arguments)]
fn collect_distractors(
    conn: &Connection,
    sql: &str,
    params: &[&dyn rusqlite::ToSql],
    correct: &str,
    limit: i64,
    pool: &mut Vec<String>,
    seen: &mut std::collections::HashSet<String>,
    question_level: i64,
    target_word: &str,
    target_band: i64,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("准备干扰项查询失败: {e}"))?;
    let rows = stmt
        .query_map(params, |r| {
            Ok(Candidate {
                answer: r.get(0)?,
                word: r.get(1)?,
                band: r.get(2)?,
            })
        })
        .map_err(|e| format!("查询干扰项失败: {e}"))?;

    // 先全量收集再排序：相似度是候选之间的相对关系，边读边取会锁死在
    // 先到的那几条上，拿不到真正最像的
    let mut candidates: Vec<Candidate> = Vec::new();
    for row in rows {
        let c = row.map_err(|e| format!("读取干扰项失败: {e}"))?;
        if seen.contains(&c.answer) || conflicts_with(&c.answer, correct) {
            continue;
        }
        candidates.push(c);
    }

    sort_by_level(&mut candidates, question_level, target_word, target_band);

    for c in candidates {
        if pool.len() as i64 >= limit {
            break;
        }
        seen.insert(c.answer.clone());
        pool.push(c.answer);
    }
    Ok(())
}

/// 按题型对候选排序，越靠前越优先入选（contracts §6）。
fn sort_by_level(
    candidates: &mut [Candidate],
    question_level: i64,
    target_word: &str,
    target_band: i64,
) {
    match question_level {
        // Lv.2 中→英：形近词优先，逼迫精确区分拼写
        2 => candidates.sort_by_key(|c| edit_distance(&c.word, target_word)),

        // Lv.3 听音辨词：音近优先，考查听辨而非视觉记忆
        3 => candidates.sort_by_key(|c| u8::from(!sounds_similar(&c.word, target_word))),

        // Lv.4 例句挖空：频段接近优先。选项难度悬殊时靠排除法就能猜中，
        // 考不出对目标词的掌握
        4..=5 => candidates.sort_by_key(|c| (c.band - target_band).abs()),

        // Lv.1 保持 SQL 的随机序。决议 S11：初学阶段引入形近词会制造混淆记忆
        _ => {}
    }
}

/// 校验单条导入数据。契约 §8「导入校验」。
///
/// 返回 `Err` 即拒绝该条——**不静默跳过**，拒绝原因会回传给调用方。
/// 句子是否包含该词的某个词形（契约 §8）。
///
/// 取词干后要求**词边界起始**：裸 `contains` 会让 `art` 命中 `start`，
/// 把明显无关的例句判为合格。
///
/// 词干长度取 `len - 3`（下限 3），需与 `scripts/wordlist/build_library.py`
/// 的同名规则保持一致——两处曾分别用 `-2` 与 `-3`，导致构建脚本报告全部合格，
/// 而运行时静默拒掉 `overcome`（例句用了过去式 `overcame`，前 6 字符对不上）。
fn contains_word_form(sentence: &str, word: &str) -> bool {
    let base = word.split_whitespace().next().unwrap_or(word);
    let keep = base.len().saturating_sub(3).max(3);
    let stem: String = base.chars().take(keep).collect::<String>().to_lowercase();
    if stem.is_empty() {
        return false;
    }

    let lower = sentence.to_lowercase();
    lower.match_indices(&stem).any(|(i, _)| {
        i == 0 || !lower.as_bytes()[i - 1].is_ascii_alphabetic()
    })
}

pub fn validate(item: &WordImport) -> Result<(), String> {
    let w = item.word.trim();
    if w.is_empty() {
        return Err("单词为空".into());
    }
    if !w
        .chars()
        .all(|c| c.is_ascii_lowercase() || c == '-' || c == '\'' || c == ' ')
        || !w.starts_with(|c: char| c.is_ascii_lowercase())
    {
        return Err(format!("单词 `{w}` 不符合规范（应为小写英文，允许连字符/撇号/空格）"));
    }
    // 音标可缺省，但一旦提供就必须是 /.../ 形式
    let phonetic_ok = item.phonetic.is_empty()
        || (item.phonetic.starts_with('/') && item.phonetic.ends_with('/'));
    if !phonetic_ok {
        return Err(format!("音标 `{}` 未以斜杠包裹", item.phonetic));
    }
    if item.pos.trim().is_empty() {
        return Err("词性为空".into());
    }
    if item.meaning.trim().is_empty() {
        return Err("释义为空".into());
    }
    // 释义含英文字母通常意味着字段错位（把英文塞进了中文列）
    if item.meaning.chars().any(|c| c.is_ascii_alphabetic()) {
        return Err(format!("释义 `{}` 含英文字母，疑似字段错位", item.meaning));
    }
    if item.example_1.trim().is_empty() {
        return Err("例句 example_1 为空".into());
    }
    // 例句必须真的包含该词，否则等于没有例句
    if !contains_word_form(&item.example_1, w) {
        return Err(format!("例句未包含单词 `{w}` 的任何词形"));
    }
    if !(1..=5).contains(&item.frequency_band) {
        return Err(format!("frequency_band {} 越界（应为 1-5）", item.frequency_band));
    }
    if !VALID_LEVELS.contains(&item.level.as_str()) {
        return Err(format!("level `{}` 不在受控词表中", item.level));
    }
    if !VALID_ZONES.contains(&item.zone.as_str()) {
        return Err(format!("zone `{}` 不在受控词表中", item.zone));
    }
    Ok(())
}

/// 批量导入。校验失败的条目被拒绝并记录原因，不影响其余条目。
///
/// 整批在单个事务中执行：要么全部生效，要么全部回滚。
pub fn import(conn: &mut Connection, items: &[WordImport]) -> Result<ImportOutcome, String> {
    let now = clock::now();
    let mut outcome = ImportOutcome {
        inserted: 0,
        updated: 0,
        rejected: Vec::new(),
    };

    let tx = conn
        .transaction()
        .map_err(|e| format!("开启导入事务失败: {e}"))?;

    for item in items {
        if let Err(reason) = validate(item) {
            outcome.rejected.push(RejectedWord {
                word: item.word.clone(),
                reason,
            });
            continue;
        }

        let existed: bool = tx
            .query_row(
                "SELECT 1 FROM words WHERE word = ?1",
                [&item.word],
                |_| Ok(()),
            )
            .optional()
            .map_err(|e| format!("查询词条 `{}` 是否存在失败: {e}", item.word))?
            .is_some();

        tx.execute(
            "INSERT INTO words
               (word, phonetic, pos, meaning, example_1, example_2,
                level, frequency_band, frequency_rank, zone, source_edition, created_at,
                pos_2, meaning_2)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(word) DO UPDATE SET
               phonetic = excluded.phonetic,
               pos = excluded.pos,
               meaning = excluded.meaning,
               example_1 = excluded.example_1,
               example_2 = excluded.example_2,
               level = excluded.level,
               frequency_band = excluded.frequency_band,
               frequency_rank = excluded.frequency_rank,
               zone = excluded.zone,
               source_edition = excluded.source_edition,
               pos_2 = excluded.pos_2,
               meaning_2 = excluded.meaning_2",
            rusqlite::params![
                item.word,
                item.phonetic,
                item.pos,
                item.meaning,
                item.example_1,
                item.example_2,
                item.level,
                item.frequency_band,
                item.frequency_rank,
                item.zone,
                item.source_edition,
                now,
                item.pos_2,
                item.meaning_2,
            ],
        )
        .map_err(|e| format!("写入词条 `{}` 失败: {e}", item.word))?;

        if existed {
            outcome.updated += 1;
        } else {
            outcome.inserted += 1;
        }
    }

    tx.commit().map_err(|e| format!("提交导入事务失败: {e}"))?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations;
    use crate::test_support::in_memory_db;

    fn db() -> Connection {
        let mut conn = in_memory_db();
        migrations::run(&mut conn).unwrap();
        conn
    }

    fn sample(word: &str) -> WordImport {
        WordImport {
            word: word.into(),
            phonetic: "/ˈkrɪstl/".into(),
            pos: "n.".into(),
            meaning: "水晶".into(),
            pos_2: None,
            meaning_2: None,
            example_1: format!("A glowing {word} lights the cave."),
            example_2: String::new(),
            level: "junior".into(),
            frequency_band: 1,
            frequency_rank: None,
            zone: "newbie".into(),
            source_edition: "renjiao".into(),
        }
    }

    #[test]
    fn 合法词条通过校验并入库() {
        let mut conn = db();
        let outcome = import(&mut conn, &[sample("crystal")]).unwrap();
        assert_eq!(outcome.inserted, 1);
        assert_eq!(outcome.updated, 0);
        assert!(outcome.rejected.is_empty());
        assert_eq!(count(&conn).unwrap(), 1);
    }

    #[test]
    fn 重复导入同一词为更新而非新增() {
        let mut conn = db();
        import(&mut conn, &[sample("crystal")]).unwrap();

        let mut changed = sample("crystal");
        changed.meaning = "结晶体".into();
        let outcome = import(&mut conn, &[changed]).unwrap();

        assert_eq!(outcome.inserted, 0);
        assert_eq!(outcome.updated, 1);
        assert_eq!(count(&conn).unwrap(), 1);

        let w = find_by_id(&conn, 1).unwrap().unwrap();
        assert_eq!(w.meaning, "结晶体", "更新未生效");
    }

    #[test]
    fn 非法词条被拒绝且附带原因() {
        let mut conn = db();

        let mut bad_meaning = sample("alpha");
        bad_meaning.meaning = "crystal".into(); // 释义写成英文 = 字段错位

        let mut bad_band = sample("beta");
        bad_band.frequency_band = 9;

        let mut bad_zone = sample("gamma");
        bad_zone.zone = "nowhere".into();

        let mut bad_example = sample("delta");
        bad_example.example_1 = "This sentence lacks the word.".into();

        let outcome = import(
            &mut conn,
            &[bad_meaning, bad_band, bad_zone, bad_example, sample("crystal")],
        )
        .unwrap();

        assert_eq!(outcome.inserted, 1, "只有合法的那条应入库");
        assert_eq!(outcome.rejected.len(), 4);
        for r in &outcome.rejected {
            assert!(!r.reason.is_empty(), "拒绝原因不能为空");
        }
        assert_eq!(count(&conn).unwrap(), 1);
    }

    #[test]
    fn 校验规则逐条生效() {
        let mut w = sample("crystal");
        assert!(validate(&w).is_ok());

        w.word = "Crystal".into();
        assert!(validate(&w).is_err(), "大写未被拒绝");

        w = sample("crystal");
        w.phonetic = "krɪstl".into();
        assert!(validate(&w).is_err(), "缺斜杠的音标未被拒绝");

        w = sample("crystal");
        w.meaning = "".into();
        assert!(validate(&w).is_err(), "空释义未被拒绝");

        w = sample("crystal");
        w.level = "college".into();
        assert!(validate(&w).is_err(), "非法 level 未被拒绝");

        // 四级词必须放行，否则扩充词库时它们会被静默挡在门外
        w = sample("crystal");
        w.level = "cet4".into();
        assert!(validate(&w).is_ok(), "cet4 应通过校验");
    }

    #[test]
    fn 例句词形匹配容纳时态变化() {
        // 不规则动词的例句用变位形式是正常的，词干规则必须容纳
        assert!(contains_word_form("She overcame her fear of flying.", "overcome"));
        assert!(contains_word_form("They abandoned the old cart.", "abandon"));
        assert!(contains_word_form("He is running fast.", "run"));
        assert!(contains_word_form("The castles stood tall.", "castle"));
    }

    #[test]
    fn 例句词形匹配要求词边界() {
        // 裸 contains 会让 art 命中 start，把无关例句判为合格
        assert!(!contains_word_form("She will start the race.", "art"));
        assert!(contains_word_form("She studies art at school.", "art"));
        // 词干出现在词首才算，出现在词中不算
        assert!(!contains_word_form("The instrument was heavy.", "rum"));
    }

    #[test]
    fn 例句词形规则与构建脚本一致() {
        // scripts/wordlist/build_library.py 用 len-3（下限 3）。两处曾分别用
        // -2 与 -3，构建报告 3657 全合格而运行时静默拒掉 overcome
        assert!(contains_word_form("She overcame it.", "overcome"));
        assert!(!contains_word_form("Nothing matches here.", "overcome"));
    }

    #[test]
    fn 干扰项优先同词性且排除自身() {
        let mut conn = db();
        let mut items: Vec<WordImport> = (0..6)
            .map(|i| {
                let mut w = sample(&format!("noun{}", (b'a' + i) as char));
                w.meaning = format!("名词释义{i}");
                w
            })
            .collect();
        let mut verb = sample("run");
        verb.pos = "v.".into();
        verb.meaning = "跑".into();
        items.push(verb);
        import(&mut conn, &items).unwrap();

        // 同词性有 5 个可选，取 3 个时不需要降级
        let pool = distractor_pool(&conn, 1, 1, 3).unwrap();
        assert_eq!(pool.len(), 3);
        assert!(!pool.contains(&"名词释义0".to_string()), "自身不应出现");
        assert!(!pool.contains(&"跑".to_string()), "同词性充足时不应降级取其他词性");
    }

    #[test]
    fn 同词性候选不足时降级补足() {
        let mut conn = db();
        // 这个词性全库只有它自己——正是 `after` 的 prep./conj. 情况
        let mut lonely = sample("after");
        lonely.pos = "prep./conj.".into();
        lonely.meaning = "在之后".into();

        let mut items = vec![lonely];
        for i in 0..5 {
            let mut w = sample(&format!("noun{}", (b'a' + i) as char));
            w.meaning = format!("名词释义{i}");
            items.push(w);
        }
        import(&mut conn, &items).unwrap();

        let pool = distractor_pool(&conn, 1, 1, 3).unwrap();
        assert_eq!(
            pool.len(),
            3,
            "同词性无候选时必须降级补足，否则题目只剩正确答案一个选项"
        );
        assert!(!pool.contains(&"在之后".to_string()));
    }

    #[test]
    fn 干扰项排除与正确释义互为子串的候选() {
        let mut conn = db();
        let mut target = sample("after");
        target.meaning = "在之后".into();

        let mut substring = sample("later");
        substring.meaning = "之后".into(); // 是正确释义的子串

        let mut superstring = sample("afterward");
        superstring.meaning = "在之后的时间".into(); // 包含正确释义

        let mut normal = sample("cat");
        normal.meaning = "猫".into();

        import(&mut conn, &[target, substring, superstring, normal]).unwrap();

        let pool = distractor_pool(&conn, 1, 1, 3).unwrap();
        assert!(!pool.contains(&"之后".to_string()), "子串候选应被排除");
        assert!(!pool.contains(&"在之后的时间".to_string()), "超串候选应被排除");
        assert!(pool.contains(&"猫".to_string()));
    }

    #[test]
    fn 干扰项无重复() {
        let mut conn = db();
        let items: Vec<WordImport> = (0..8)
            .map(|i| {
                let mut w = sample(&format!("noun{}", (b'a' + i) as char));
                w.meaning = format!("名词释义{i}");
                w
            })
            .collect();
        import(&mut conn, &items).unwrap();

        for _ in 0..20 {
            let pool = distractor_pool(&conn, 1, 1, 3).unwrap();
            let unique: std::collections::HashSet<_> = pool.iter().collect();
            assert_eq!(unique.len(), pool.len(), "干扰项出现重复: {pool:?}");
        }
    }

    #[test]
    fn 词库过小时返回全部可用候选而不报错() {
        let mut conn = db();
        let mut a = sample("alpha");
        a.meaning = "甲".into();
        let mut b = sample("beta");
        b.meaning = "乙".into();
        import(&mut conn, &[a, b]).unwrap();

        // 全库只有 2 个词，最多只能给出 1 个干扰项
        let pool = distractor_pool(&conn, 1, 1, 3).unwrap();
        assert_eq!(pool.len(), 1);
        assert_eq!(pool[0], "乙");
    }

    #[test]
    fn 搜索命中单词与释义() {
        let mut conn = db();
        import(&mut conn, &[sample("crystal")]).unwrap();

        assert_eq!(search(&conn, "cryst", 10).unwrap().len(), 1);
        assert_eq!(search(&conn, "水晶", 10).unwrap().len(), 1);
        assert_eq!(search(&conn, "不存在的词", 10).unwrap().len(), 0);
    }

    #[test]
    fn 空库查询返回空而非报错() {
        let conn = db();
        assert_eq!(count(&conn).unwrap(), 0);
        assert!(find_by_id(&conn, 1).unwrap().is_none());
        assert!(search(&conn, "x", 10).unwrap().is_empty());
    }

    // ── 题型分级（contracts §6）──────────────────────────

    /// 造一批同词性、拼写差异可控的词，用于观察排序策略。
    fn seed_for_levels(conn: &mut Connection) {
        let specs = [
            // (word, meaning, band) —— adapt/adopt/adept 互为形近词
            ("adapt", "适应", 1),
            ("adopt", "采用", 2),
            ("adept", "熟练的", 4),
            ("mountain", "山脉", 5),
            ("bicycle", "自行车", 5),
            ("umbrella", "雨伞", 5),
            ("acquire", "获得", 1),
        ];
        let items: Vec<WordImport> = specs
            .iter()
            .map(|(w, m, b)| {
                let mut item = sample(w);
                item.meaning = (*m).into();
                item.frequency_band = *b;
                item.pos = "v.".into();
                item
            })
            .collect();
        import(conn, &items).unwrap();
    }

    #[test]
    fn 一级题型返回释义二级以上返回单词() {
        let mut conn = db();
        seed_for_levels(&mut conn);

        // Lv.1：看英文选中文，选项必须是释义
        let lv1 = distractor_pool(&conn, 1, 1, 3).unwrap();
        assert!(
            lv1.iter().all(|s| !s.is_ascii()),
            "Lv.1 的干扰项应为中文释义，实际: {lv1:?}"
        );

        // Lv.2：看中文选英文，选项必须是单词。搞反了题目会变成「看中文选中文」
        let lv2 = distractor_pool(&conn, 1, 2, 3).unwrap();
        assert!(
            lv2.iter().all(|s| s.is_ascii()),
            "Lv.2 的干扰项应为英文单词，实际: {lv2:?}"
        );

        for level in [3, 4, 5] {
            let pool = distractor_pool(&conn, 1, level, 3).unwrap();
            assert!(
                pool.iter().all(|s| s.is_ascii()),
                "Lv.{level} 的干扰项应为英文单词，实际: {pool:?}"
            );
        }
    }

    #[test]
    fn 二级题型优先选形近词() {
        let mut conn = db();
        seed_for_levels(&mut conn);

        // adapt 的形近词是 adopt(1) / adept(2)，远于 mountain 等
        let pool = distractor_pool(&conn, 1, 2, 2).unwrap();
        assert!(
            pool.contains(&"adopt".to_string()) || pool.contains(&"adept".to_string()),
            "Lv.2 应优先取形近词，实际: {pool:?}"
        );
    }

    #[test]
    fn 一级题型不引入形近词() {
        let mut conn = db();
        seed_for_levels(&mut conn);

        // 决议 S11：adapt/adopt/adept 摆在一起会让初学者建立混淆记忆。
        // 多跑几轮确认 Lv.1 是随机而非按相似度——若按相似度排，
        // 形近词的释义会每次都出现
        let mut near_hits = 0;
        for _ in 0..30 {
            let pool = distractor_pool(&conn, 1, 1, 2).unwrap();
            if pool.contains(&"采用".to_string()) && pool.contains(&"熟练的".to_string()) {
                near_hits += 1;
            }
        }
        assert!(
            near_hits < 25,
            "Lv.1 每轮都取到形近词的释义（{near_hits}/30），说明误用了相似度排序"
        );
    }

    #[test]
    fn 四级题型优先选同频段词() {
        let mut conn = db();
        seed_for_levels(&mut conn);

        // acquire 是 band 1，同为 band 1 的只有 adapt；band 5 的三个词最远。
        // 挖空题里若混入难度悬殊的选项，排除法就能猜中
        let acquire_id = conn
            .query_row("SELECT id FROM words WHERE word = 'acquire'", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap();
        let pool = distractor_pool(&conn, acquire_id, 4, 2).unwrap();
        assert!(
            pool.contains(&"adapt".to_string()),
            "Lv.4 应优先取同频段词，实际: {pool:?}"
        );
    }

    #[test]
    fn 编辑距离计算正确() {
        assert_eq!(edit_distance("adapt", "adopt"), 1);
        assert_eq!(edit_distance("adapt", "adept"), 1);
        assert_eq!(edit_distance("adapt", "adapt"), 0);
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("abc", ""), 3);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn 音近判定要求首字母相同且长度接近() {
        assert!(sounds_similar("cat", "cap"));
        assert!(sounds_similar("crystal", "crown"));
        assert!(!sounds_similar("cat", "dog"), "首字母不同不算音近");
        assert!(!sounds_similar("a", "abandon"), "长度悬殊不算音近");
    }

    #[test]
    fn 各题型都保证选项不重复且不含正确答案() {
        let mut conn = db();
        seed_for_levels(&mut conn);

        for level in 1..=5 {
            let pool = distractor_pool(&conn, 1, level, 3).unwrap();
            let unique: std::collections::HashSet<_> = pool.iter().collect();
            assert_eq!(unique.len(), pool.len(), "Lv.{level} 干扰项重复: {pool:?}");

            let answer = if level == 1 { "适应" } else { "adapt" };
            assert!(
                !pool.contains(&answer.to_string()),
                "Lv.{level} 干扰项含正确答案"
            );
        }
    }

    #[test]
    fn 干扰项查询对不存在的词条报错() {
        let conn = db();
        // 与「空库返回空」不同：word_id 指向不存在的词是调用方的逻辑错误。
        // 静默返回空会让题目只剩一个选项，且没有任何线索指向根因
        let err = distractor_pool(&conn, 999, 1, 3).unwrap_err();
        assert!(err.contains("999"), "错误消息应指明是哪个词条: {err}");
    }
    #[test]
    fn 真实四级词条能通过导入() {
        let mut conn = crate::test_support::in_memory_db();
        crate::db::migrations::run(&mut conn).unwrap();
        let item = WordImport {
            word: "program".into(),
            phonetic: "/'prәugræm/".into(),
            pos: "n.".into(),
            meaning: "节目，节目单，程序".into(),
            pos_2: None,
            meaning_2: None,
            example_1: "Our sandbox program lets you build with colored blocks.".into(),
            example_2: "The racing program on TV starts at eight tonight.".into(),
            level: "cet4".into(),
            frequency_band: 1,
            frequency_rank: None,
            zone: "water".into(),
            source_edition: "gk".into(),
        };
        let out = import(&mut conn, &[item]).unwrap();
        assert!(out.rejected.is_empty(), "真实四级词条被拒: {:?}", out.rejected);
    }

    #[test]
    fn 第二词性能往返() {
        let mut conn = db();
        let mut w = sample("train");
        w.pos = "n.".into();
        w.meaning = "火车，列车".into();
        w.pos_2 = Some("vt.".into());
        w.meaning_2 = Some("训练，教育".into());
        import(&mut conn, &[w]).unwrap();

        let got = search(&conn, "train", 1).unwrap().pop().unwrap();
        assert_eq!(got.pos_2.as_deref(), Some("vt."));
        assert_eq!(got.meaning_2.as_deref(), Some("训练，教育"));
    }

    #[test]
    fn 没有第二词性时读回来是空() {
        let mut conn = db();
        import(&mut conn, &[sample("listen")]).unwrap();
        let got = search(&conn, "listen", 1).unwrap().pop().unwrap();
        // null 就是「没有」。用空串伪装成有，界面会渲染一行空的「另见：」
        assert_eq!(got.pos_2, None);
        assert_eq!(got.meaning_2, None);
    }

    #[test]
    fn 重新导入能清掉不再适用的第二词性() {
        let mut conn = db();
        let mut w = sample("plant");
        w.pos_2 = Some("vt.".into());
        w.meaning_2 = Some("种植".into());
        import(&mut conn, &[w.clone()]).unwrap();

        w.pos_2 = None;
        w.meaning_2 = None;
        import(&mut conn, &[w]).unwrap();

        // upsert 漏掉这两列的话，词库改了主意也清不掉旧值——
        // 界面会一直显示一个已经被判定为不常用的义项
        let got = search(&conn, "plant", 1).unwrap().pop().unwrap();
        assert_eq!(got.pos_2, None);
    }
}
