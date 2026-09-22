#!/usr/bin/env bash
# verify.sh —— Kanesumi 一键验证：全量测试 + clippy 与基线逐条比对。
#
# 为什么需要它：本仓的 clippy 有**既存告警**（CJK 注释按双宽计宽导致的折行等），
# 规矩是「不得新增」。手工比对容易漏，脚本把这件事固化：
# 输出「目标 → 告警数」的清单，与 scripts/clippy-baseline.txt 比对，多一条即失败。
#
# 用法：
#   ./scripts/verify.sh                # 测试 + clippy 比对
#   ./scripts/verify.sh --update-baseline   # 把当前 clippy 结果写成新基线（**仅在有意消/加告警时用**）
#
# 注：Windows 侧有对应的 verify.ps1，两者输出格式一致，基线文件共用。

set -uo pipefail
cd "$(dirname "$0")/.."

BASELINE="scripts/clippy-baseline.txt"
FAILED=0

echo "== 1/3 全量测试（cargo test --workspace）=="
TEST_OUT="$(cargo test --workspace 2>&1)"
PASSED="$(printf '%s\n' "$TEST_OUT" | grep -oE 'test result: ok\. [0-9]+ passed' | grep -oE '[0-9]+' | paste -sd+ - | bc)"
FAILED_SUITES="$(printf '%s\n' "$TEST_OUT" | grep -c 'test result: FAILED')"
if [ "$FAILED_SUITES" != "0" ]; then
  echo "✗ 有 $FAILED_SUITES 个测试目标失败："
  printf '%s\n' "$TEST_OUT" | grep -E 'test result: FAILED|^test .* FAILED' | head -20
  FAILED=1
else
  echo "✓ 全部通过，合计 $PASSED 项"
fi

echo
echo "== 2/3 测试字体可用性 =="
if [ -n "${KANESUMI_TEST_FONT:-}" ] && [ -f "${KANESUMI_TEST_FONT}" ]; then
  echo "✓ KANESUMI_TEST_FONT=$KANESUMI_TEST_FONT"
else
  FOUND=""
  for p in /usr/local/share/fonts/s/SourceHanSansSC-Regular.otf \
           /usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc \
           /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
           /usr/share/fonts/TTF/DejaVuSans.ttf \
           /mnt/c/Windows/Fonts/msyh.ttc; do
    [ -f "$p" ] && FOUND="$p" && break
  done
  if [ -n "$FOUND" ]; then
    echo "✓ 命中系统字体：$FOUND（未设 KANESUMI_TEST_FONT 也能跑）"
  else
    echo "⚠ 未找到任何测试字体：字体相关测试会**静默跳过**，等于没跑（设 KANESUMI_TEST_FONT 指定）"
    FAILED=1
  fi
fi

echo
echo "== 3/3 clippy 与基线比对（cargo clippy --workspace --all-targets）=="
clippy_summary() {
  grep -oE '^warning: `[^`]+` generated [0-9]+ warning' \
    | sed -E 's/^warning: `(.+)` generated ([0-9]+) warning/\1  \2/' \
    | sort
}
CURRENT="$(cargo clippy --workspace --all-targets 2>&1 | clippy_summary)"

# clippy **命中缓存时不重复输出告警**（只打印 Finished）——那会把基线静默写成空。
# 判据：一条摘要都没有时，bump 各 crate 的 lib.rs mtime 强制重编一次再取（只改 mtime，不动内容）。
if [ -z "$CURRENT" ]; then
  echo "（clippy 无输出 → 命中缓存，强制重编一次以取真实告警）"
  for d in kanesumi-*/; do
    [ -f "${d}src/lib.rs" ] && touch "${d}src/lib.rs"
  done
  CURRENT="$(cargo clippy --workspace --all-targets 2>&1 | clippy_summary)"
fi

if [ "${1:-}" = "--update-baseline" ]; then
  printf '%s\n' "$CURRENT" > "$BASELINE"
  echo "已写入基线 $BASELINE ："
  printf '%s\n' "$CURRENT" | sed 's/^/  /'
  exit 0
fi

if [ ! -f "$BASELINE" ]; then
  echo "⚠ 基线文件 $BASELINE 不存在 —— 先跑一次 --update-baseline 固化当前状态"
  FAILED=1
else
  if diff -u "$BASELINE" <(printf '%s\n' "$CURRENT") > /tmp/kanesumi-clippy-diff.txt 2>/dev/null; then
    echo "✓ 告警数与基线逐条一致（$(printf '%s\n' "$CURRENT" | wc -l | tr -d ' ') 个目标）"
  else
    echo "✗ clippy 告警与基线不一致（- 基线 / + 现在）："
    cat /tmp/kanesumi-clippy-diff.txt
    FAILED=1
  fi
fi

echo
[ "$FAILED" = "0" ] && echo "== 验证通过 ==" || echo "== 验证未通过（见上）=="
exit "$FAILED"
