#!/usr/bin/env bash
# verify.sh —— Kanesumi 一键验证：全量测试 + clippy 与基线逐条比对。
#
# 为什么需要它：本仓的 clippy 有**既存告警**（CJK 注释按双宽计宽导致的折行等），
# 规矩是「不得新增」。手工比对容易漏，脚本把这件事固化：
# 输出「目标 → 告警数」的清单，与 scripts/clippy-baseline.txt 比对，多一条即失败。
#
# 用法：
#   ./scripts/verify.sh                     # 测试 + clippy 比对（bash scripts/verify.sh 亦可）
#   ./scripts/verify.sh --update-baseline   # 用当前 clippy 结果重写基线（沿用已有平台标签）
#
# 基线格式：每行「<目标>  <告警数>」，末尾可带 `@<平台>` 表示**只在该平台计入**。
# 为什么需要平台标签：`kanesumi-harness` 的 Wayland + wgpu 外壳在非 Linux 上被 cfg 门控掉，
# 两端编出来的告警集本来就不同 —— 一份不分平台的基线不可能同时在两端通过。
#
# 注：Windows 侧有对应的 verify.ps1，两者共用同一份基线（都认平台标签）。

set -uo pipefail
cd "$(dirname "$0")/.."

BASELINE="scripts/clippy-baseline.txt"
STATE_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/kanesumi"
DIFF_OUT="$STATE_DIR/clippy-diff.txt"
FAILED=0

case "$(uname -s 2>/dev/null || echo unknown)" in
  Linux)                OS_TAG=linux ;;
  Darwin)               OS_TAG=darwin ;;
  MINGW*|MSYS*|CYGWIN*) OS_TAG=windows ;;
  *)                    OS_TAG=unknown ;;
esac

echo "== 1/3 全量测试（cargo test --workspace）=="
TEST_OUT="$(cargo test --workspace 2>&1)"
PASSED="$(printf '%s\n' "$TEST_OUT" | grep -oE 'test result: ok\. [0-9]+ passed' | grep -oE '[0-9]+' | awk '{ s += $1 } END { print s + 0 }')"
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
  # clippy 的摘要行是「warning: `crate` (lib test) generated 36 warnings (…后缀…)」——
  # 结束反引号后面跟的是 "(kind)" 而不是 " generated"，所以模式必须从 "warning: " 抓到
  # " generated"，不能写成 `[^`]+` generated`（那样一条都匹配不到）。
  # 末尾的 "(N duplicates) (run `cargo clippy --fix …`)" 也要整段吃掉。
  # LC_ALL=C：基线由 Windows 侧的 Sort-Object 排过，glibc 的 en_US 排序会忽略空格，
  # 两侧排序口径不一致会得到「内容相同、顺序不同」的假差异。
  sed -nE 's/^warning: (.+) generated ([0-9]+) warning.*$/\1  \2/p' | LC_ALL=C sort
}

# 基线读取：带 `@<平台>` 标签的行只在本平台计入，其余各平台都计入；
# 首行的 UTF-8 BOM 与 CRLF 一并归一（基线可能由 Windows 侧的 PS 5.1 写入）。
baseline_for_os() {
  sed -e '1s/^\xEF\xBB\xBF//' -e 's/\r$//' "$BASELINE" |
    awk -v os="$1" '
      /^[[:space:]]*$/ { next }
      /^[[:space:]]*#/ { next }
      {
        line = $0
        if (match(line, /@[A-Za-z]+[[:space:]]*$/)) {
          tag = substr(line, RSTART + 1, RLENGTH - 1); gsub(/[^A-Za-z]/, "", tag)
          if (tag != os) next
          line = substr(line, 1, RSTART - 1); sub(/[[:space:]]+$/, "", line)
        }
        print line
      }' |
    LC_ALL=C sort
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
  CUR_FILE="$(mktemp)"
  printf '%s\n' "$CURRENT" > "$CUR_FILE"
  NEW_FILE="$(mktemp)"
  [ -f "$BASELINE" ] || : > "$BASELINE"
  # 标签按**目标名**沿用（不看告警数，否则一改数就丢标签）；新出现的目标默认各平台都算。
  awk -v cur="$CUR_FILE" '
    NR == FNR {
      if ($0 ~ /^[[:space:]]*#/ || $0 ~ /^[[:space:]]*$/) next
      tag = ""
      if (match($0, /@[A-Za-z]+[[:space:]]*$/)) {
        tag = substr($0, RSTART, RLENGTH); gsub(/[[:space:]]/, "", tag)
        $0 = substr($0, 1, RSTART - 1); sub(/[[:space:]]+$/, "", $0)
      }
      lbl = $0; sub(/  [0-9]+$/, "", lbl)
      if (tag != "") wastag[lbl] = tag
      next
    }
    { next }
    END {
      print "# clippy 告警基线：每行「<目标>  <告警数>」，末尾的 @<平台> 表示只在该平台计入。"
      print "# 平台标签由 scripts/verify.sh --update-baseline 沿用（按目标匹配，不看告警数）。"
      while ((getline l < cur) > 0) {
        if (l == "") continue
        lbl = l; sub(/  [0-9]+$/, "", lbl)
        if (lbl in wastag) print l "  " wastag[lbl]
        else print l
      }
    }
  ' "$BASELINE" /dev/null > "$NEW_FILE"
  mv "$NEW_FILE" "$BASELINE"
  rm -f "$CUR_FILE"
  echo "已写入基线 $BASELINE （当前平台：$OS_TAG）："
  sed 's/^/  /' "$BASELINE"
  exit 0
fi

if [ ! -f "$BASELINE" ]; then
  echo "⚠ 基线文件 $BASELINE 不存在 —— 先跑一次 --update-baseline 固化当前状态"
  FAILED=1
else
  mkdir -p "$STATE_DIR"
  if diff -u <(baseline_for_os "$OS_TAG") <(printf '%s\n' "$CURRENT") > "$DIFF_OUT" 2>/dev/null; then
    echo "✓ 告警数与基线逐条一致（当前平台 $OS_TAG，$(printf '%s\n' "$CURRENT" | grep -c .) 个目标）"
  else
    echo "✗ clippy 告警与基线不一致（- 基线 / + 现在；完整差异见 $DIFF_OUT）："
    cat "$DIFF_OUT"
    FAILED=1
  fi
fi

echo
[ "$FAILED" = "0" ] && echo "== 验证通过 ==" || echo "== 验证未通过（见上）=="
exit "$FAILED"
