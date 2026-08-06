#!/usr/bin/env python3
"""把旧数据库的学习进度迁移到新词库。

背景：T18 更换词库时清空了数据库，学习记录留在备份里。`word_states` 与
`review_logs` 通过 `word_id` 关联词条，而两个库的 id 分配完全不同——
旧库 1 号是 ability，新库 1 号是 the。直接拷贝会张冠李戴，必须按单词文本重映射。

用法：
    python3 scripts/restore_progress.py <旧库路径> <新库路径>
    python3 scripts/restore_progress.py <旧库> <新库> --apply    # 实际写入
"""

import argparse
import shutil
import sqlite3
import sys
from datetime import datetime
from pathlib import Path


def build_id_map(old: sqlite3.Connection, new: sqlite3.Connection) -> tuple[dict[int, int], list[str]]:
    """按单词文本建立 旧 word_id → 新 word_id 的映射。"""
    new_ids = {word: wid for wid, word in new.execute("SELECT id, word FROM words")}
    mapping: dict[int, int] = {}
    missing: list[str] = []
    for old_id, word in old.execute("SELECT id, word FROM words"):
        if word in new_ids:
            mapping[old_id] = new_ids[word]
        else:
            missing.append(word)
    return mapping, missing


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("old_db")
    ap.add_argument("new_db")
    ap.add_argument("--apply", action="store_true", help="实际写入，否则仅预览")
    args = ap.parse_args()

    old_path, new_path = Path(args.old_db), Path(args.new_db)
    if not old_path.exists() or not new_path.exists():
        sys.exit("数据库路径不存在")

    if args.apply:
        # 迁移前先备份目标库——恢复脚本本身出错时要能退回
        stamp = datetime.now().strftime("%Y%m%dT%H%M%S")
        backup = new_path.with_suffix(f".pre-restore-{stamp}.db")
        shutil.copy2(new_path, backup)
        print(f"目标库已备份 → {backup.name}")

    old = sqlite3.connect(old_path)
    new = sqlite3.connect(new_path)
    new.execute("PRAGMA foreign_keys = ON")

    id_map, missing = build_id_map(old, new)
    print(f"词条映射：{len(id_map)} 命中，{len(missing)} 缺失" + (f"（{', '.join(missing)}）" if missing else ""))

    # ── word_states ──
    states = old.execute("""
        SELECT word_id, difficulty, stability, due_at, fsrs_state, app_state,
               reps, lapses, question_level, reinforce_streak, last_review_at, mastered_at
        FROM word_states
    """).fetchall()

    migrated = skipped = kept_newer = 0
    for row in states:
        old_id = row[0]
        if old_id not in id_map:
            skipped += 1
            continue
        new_id = id_map[old_id]

        # 新库可能已有该词的学习记录（删库后又练过）。取 reps 更大的那份，
        # 它代表更完整的学习历史
        existing = new.execute("SELECT reps FROM word_states WHERE word_id = ?", (new_id,)).fetchone()
        if existing and existing[0] >= row[6]:
            kept_newer += 1
            continue

        new.execute(
            """INSERT INTO word_states
               (word_id, difficulty, stability, due_at, fsrs_state, app_state,
                reps, lapses, question_level, reinforce_streak, last_review_at, mastered_at)
               VALUES (?,?,?,?,?,?,?,?,?,?,?,?)
               ON CONFLICT(word_id) DO UPDATE SET
                 difficulty=excluded.difficulty, stability=excluded.stability,
                 due_at=excluded.due_at, fsrs_state=excluded.fsrs_state,
                 app_state=excluded.app_state, reps=excluded.reps,
                 lapses=excluded.lapses, question_level=excluded.question_level,
                 reinforce_streak=excluded.reinforce_streak,
                 last_review_at=excluded.last_review_at, mastered_at=excluded.mastered_at""",
            (new_id, *row[1:]),
        )
        migrated += 1

    # ── review_logs ──
    # session_id 置空：旧库的会话 id 在新库无对应行，保留会违反外键。
    # 日志的价值在于算法回溯（前后快照），会话归属是次要信息
    logs = old.execute("""
        SELECT word_id, question_type, is_correct, reaction_ms, rating,
               difficulty_before, stability_before, difficulty_after, stability_after, reviewed_at
        FROM review_logs
    """).fetchall()
    log_count = 0
    for row in logs:
        if row[0] not in id_map:
            continue
        new.execute(
            """INSERT INTO review_logs
               (word_id, session_id, question_type, is_correct, reaction_ms, rating,
                difficulty_before, stability_before, difficulty_after, stability_after, reviewed_at)
               VALUES (?, NULL, ?,?,?,?,?,?,?,?,?)""",
            (id_map[row[0]], *row[1:]),
        )
        log_count += 1

    # ── player_stats ──
    # XP 累加（两段都是真实获得的），streak 取较大值——它代表实际达成过的记录
    o = old.execute("SELECT total_xp, current_streak, best_streak, makeup_cards, draw_tickets FROM player_stats WHERE id=1").fetchone()
    n = new.execute("SELECT total_xp, current_streak, best_streak, makeup_cards, draw_tickets FROM player_stats WHERE id=1").fetchone()
    total_xp = o[0] + n[0]
    level = min(int((total_xp / 50) ** 0.5) + 1, 100)
    new.execute(
        """UPDATE player_stats SET total_xp=?, level=?, current_streak=?, best_streak=?,
           makeup_cards=?, draw_tickets=? WHERE id=1""",
        (total_xp, level, max(o[1], n[1]), max(o[2], n[2]), max(o[3], n[3]), max(o[4], n[4])),
    )

    # ── sessions / daily_records ──
    sess = old.execute("""
        SELECT date, session_type, planned_count, completed_count, is_completed,
               xp_earned, postpone_count, started_at, finished_at FROM sessions
    """).fetchall()
    for row in sess:
        new.execute(
            """INSERT INTO sessions (date, session_type, planned_count, completed_count,
               is_completed, xp_earned, postpone_count, started_at, finished_at)
               VALUES (?,?,?,?,?,?,?,?,?)
               ON CONFLICT(date, session_type) DO NOTHING""", row)

    recs = old.execute("SELECT date, is_paused, eligible_count, completed_count, streak_outcome FROM daily_records").fetchall()
    for row in recs:
        new.execute(
            """INSERT INTO daily_records (date, is_paused, eligible_count, completed_count, streak_outcome)
               VALUES (?,?,?,?,?) ON CONFLICT(date) DO NOTHING""", row)

    print(f"词状态：迁移 {migrated}，跳过 {skipped}（新库中无此词），保留新库版本 {kept_newer}")
    print(f"作答日志：迁移 {log_count} 条")
    print(f"XP：{o[0]} + {n[0]} = {total_xp}（Lv.{level}）")
    print(f"连续天数：max({o[1]}, {n[1]}) = {max(o[1], n[1])}")
    print(f"会话：{len(sess)} 条，每日记录：{len(recs)} 条")

    if args.apply:
        new.commit()
        print("\n✅ 已写入")
    else:
        new.rollback()
        print("\n（预览模式，未写入。加 --apply 执行）")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
