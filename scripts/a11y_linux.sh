#!/usr/bin/env bash
# a11y_linux —— 无障碍（AT-SPI）运行时冒烟：一次跑完，零交互，结果全部落盘。
#
# 判据（两条都要成立才算通过）：
#   1. ~/.local/state/kanesumi/a11y-smoke.log 里有无障碍树，且含对话框的三个按钮节点；
#   2. ~/.local/state/kanesumi/a11y-atspi.log 里，AT-SPI 注册表能列出我们的应用，
#      且通过 D-Bus 读到的名字/角色与我们树里的一致。
#
# 用法（任何 Linux，无需图形会话）：
#   bash scripts/a11y_linux.sh
#   跨机跑：ssh <host> 'cd <repo> && bash scripts/a11y_linux.sh'
#
# AT-SPI 活在**会话总线**上，所以脚本自套一层 dbus-run-session（无图形会话的 SSH 里
# 没有总线）。目标机器需要 at-spi2-core（提供注册表服务）与 dbus。

set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

# 没有会话总线就自己开一个（AT-SPI 就活在这条总线上）。
if [ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ]; then
    exec dbus-run-session -- bash "$0" "$@"
fi

export PATH="$HOME/.cargo/bin:$PATH"
OUT="${HOME}/.local/state/kanesumi"
mkdir -p "$OUT" || exit 1
BUS_LOG="$OUT/a11y-atspi.log"
APP_STDOUT="$OUT/a11y-smoke.stdout"
: >"$BUS_LOG"

say() { echo "$*" | tee -a "$BUS_LOG"; }

say "== 构建示例 =="
if ! cargo build -q -p kanesumi-harness --features a11y --example atspi_smoke 2>>"$BUS_LOG"; then
    say "构建失败，见上方日志"
    exit 1
fi
BIN="${CARGO_TARGET_DIR:-target}/debug/examples/atspi_smoke"
[ -x "$BIN" ] || { say "找不到示例二进制：$BIN"; exit 1; }

say "== 启动示例（后台）=="
A11Y_SMOKE_SECS="${A11Y_SMOKE_SECS:-20}" "$BIN" >"$APP_STDOUT" 2>&1 &
APP=$!

# 给适配器留出「连总线 + 等激活」的时间。
sleep 8

REG_PATH=/org/a11y/atspi/accessible/root
say "== 会话总线上的 AT-SPI 服务 =="
busctl --user list --no-pager 2>&1 | grep -Ei 'atspi|accessib' | tee -a "$BUS_LOG"

say "== 注册表 GetChildren（登记在册的应用）=="
CHILDREN=$(gdbus call --session --dest org.a11y.atspi.Registry \
    --object-path "$REG_PATH" --method org.a11y.atspi.Accessible.GetChildren 2>&1)
echo "$CHILDREN" | tee -a "$BUS_LOG"

# 只在注册表列出的应用里找，不去扫全总线的 :1.x（那里有几十个无关服务）。
say "== 逐个应用读名字 / 角色 =="
for svc in $(echo "$CHILDREN" | grep -oE "':[0-9]+\.[0-9]+'" | tr -d "'" | sort -u); do
    name=$(gdbus call --session --dest "$svc" --object-path "$REG_PATH" \
        --method org.a11y.atspi.Accessible.GetName 2>/dev/null)
    role=$(gdbus call --session --dest "$svc" --object-path "$REG_PATH" \
        --method org.a11y.atspi.Accessible.GetRoleName 2>/dev/null)
    say "  $svc name=$name role=$role"
    # 子元素：能读到「确定 / 保存（默认）/ 取消」说明树真的过去了。
    kids=$(gdbus call --session --dest "$svc" --object-path "$REG_PATH" \
        --method org.a11y.atspi.Accessible.GetChildren 2>/dev/null)
    say "    children=$kids"
    for kid in $(echo "$kids" | grep -oE "'/[^']+'" | tr -d "'" | sort -u | head -6); do
        kname=$(gdbus call --session --dest "$svc" --object-path "$kid" \
            --method org.a11y.atspi.Accessible.GetName 2>/dev/null)
        krole=$(gdbus call --session --dest "$svc" --object-path "$kid" \
            --method org.a11y.atspi.Accessible.GetRoleName 2>/dev/null)
        say "      $kid name=$kname role=$krole"
    done
done

wait "$APP"
rc=$?
say "== 示例退出 rc=$rc，产物：=="
say "  树摘要  $OUT/a11y-smoke.log"
say "  stdout  $APP_STDOUT"
say "  总线侧  $BUS_LOG"
exit $rc
