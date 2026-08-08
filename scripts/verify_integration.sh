#!/usr/bin/env bash
# 集成验证：直接查真实数据库与产物，不看测试结果。
#
# 存在的理由见 integration-discipline.md §2.1：单测全绿、类型检查通过、
# spec 逐项打钩，都不能证明应用真的能用。这个脚本只问一件事——
# **磁盘上和数据库里，东西是不是真的在那儿**。
#
# 用法：bash scripts/verify_integration.sh

set -uo pipefail

DB="$HOME/Library/Application Support/com.wordcraft.app/wordcraft.db"
AUDIO="wordcraft/src-tauri/audio"
PASS=0
FAIL=0

check() {
  local desc="$1" actual="$2" expect="$3"
  if [ "$actual" = "$expect" ]; then
    printf "  ✅ %-34s %s\n" "$desc" "$actual"
    PASS=$((PASS + 1))
  else
    printf "  ❌ %-34s %s（期望 %s）\n" "$desc" "$actual" "$expect"
    FAIL=$((FAIL + 1))
  fi
}

check_min() {
  local desc="$1" actual="$2" min="$3"
  if [ "${actual:-0}" -ge "$min" ] 2>/dev/null; then
    printf "  ✅ %-34s %s\n" "$desc" "$actual"
    PASS=$((PASS + 1))
  else
    printf "  ❌ %-34s %s（应 ≥ %s）\n" "$desc" "${actual:-无}" "$min"
    FAIL=$((FAIL + 1))
  fi
}

echo "═══ WordCraft 集成验证 ═══"
echo ""

if [ ! -f "$DB" ]; then
  echo "❌ 数据库不存在：$DB"
  echo "   先启动一次应用让它初始化。"
  exit 1
fi

echo "▸ 数据库"
check "schema 版本" "$(sqlite3 "$DB" 'SELECT MAX(version) FROM schema_migrations;')" "5"
check_min "词库词数" "$(sqlite3 "$DB" 'SELECT COUNT(*) FROM words;')" 3600
check_min "卡池张数" "$(sqlite3 "$DB" 'SELECT COUNT(*) FROM cards;')" 24
check "缺来源的卡（spec F12）" "$(sqlite3 "$DB" "SELECT COUNT(*) FROM cards WHERE source='';")" "0"
check "缺例句的词" "$(sqlite3 "$DB" "SELECT COUNT(*) FROM words WHERE example_1='';")" "0"

echo ""
echo "▸ 预生成音频"
COUNT=$(ls "$AUDIO"/*.mp3 2>/dev/null | wc -l | tr -d ' ')
check_min "mp3 数量" "$COUNT" 3600
# 0 字节文件播放时无声却不报错，是最难排查的一类故障
TINY=$(find "$AUDIO" -name '*.mp3' -size -512c 2>/dev/null | wc -l | tr -d ' ')
check "过小文件（<512 字节）" "$TINY" "0"
# 高频词必须有音频，否则最常出现的词反而没声音
for w in the water apply knowledge; do
  [ -f "$AUDIO/$w.mp3" ] && R=有 || R=无
  check "高频词 $w 的音频" "$R" "有"
done

echo ""
echo "▸ 素材"
check_min "生物卡 PNG" "$(ls wordcraft/public/cards/creatures/*.png 2>/dev/null | wc -l | tr -d ' ')" 16
check_min "名画卡 PNG" "$(ls wordcraft/public/cards/paintings/*.png 2>/dev/null | wc -l | tr -d ' ')" 8

echo ""
echo "▸ 假实现残留"
# 业务代码里不该有未登记的 TODO；MOCKS.md 之外的一律算债务
STRAY=$(grep -rn "TODO\|FIXME\|XXX" wordcraft/src wordcraft/src-tauri/src 2>/dev/null \
        | grep -v "TODO(T" | wc -l | tr -d ' ')
check "未登记的 TODO/FIXME" "$STRAY" "0"
check "legacy 模块" "$([ -f wordcraft/src-tauri/src/db/legacy.rs ] && echo 存在 || echo 已删)" "已删"
check "fsrs_engine 模块" "$([ -f wordcraft/src-tauri/src/fsrs_engine.rs ] && echo 存在 || echo 已删)" "已删"

echo ""
echo "═══ $PASS 项通过，$FAIL 项失败 ═══"
[ "$FAIL" -eq 0 ] || exit 1
