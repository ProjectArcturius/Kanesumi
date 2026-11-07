#!/usr/bin/env bash
# a11y_linux —— 无障碍（AT-SPI）运行时冒烟：一次跑完，零交互，结果全部落盘。
#
# 判据（两条都要成立才算通过）：
#   1. ~/.local/state/kanesumi/a11y-smoke.log 里有无障碍树，含对话框的三个按钮节点；
#   2. ~/.local/state/kanesumi/a11y-atspi.log 里，AT-SPI 注册表能列出我们的应用，
#      且通过 D-Bus 读到的名字 / 角色与树里的一致。
#
# 用法（任何 Linux，无需图形会话）：
#   bash scripts/a11y_linux.sh
#   跨机跑：ssh <host> 'cd <repo> && bash scripts/a11y_linux.sh'
#
# 两条总线要分清（这是本脚本最容易搞错的地方）：
#   - **会话总线**：dbus-run-session 给的那条；SSH 里默认没有，所以脚本自套一层。
#   - **a11y 总线**：AT-SPI 真正住在的地方，由 at-spi-bus-launcher 另起一条私有总线。
#     应用靠 `AT_SPI_BUS_ADDRESS` 或 `org.a11y.Bus.GetAddress` 找到它 —— 所以必须
#     **先**起总线、导出地址，**再**启动被测进程。

set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

# 没有会话总线就自己开一个（a11y 总线由它再派生）。
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

REG_PATH=/org/a11y/atspi/accessible/root

say "== 起 a11y 总线（org.a11y.Bus → at-spi-bus-launcher）=="
A11Y_BUS=$(gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
    --method org.a11y.Bus.GetAddress 2>&1 | sed -n "s/^('\(.*\)',$/\1/p")
if [ -n "${A11Y_BUS:-}" ]; then
    say "  address=$A11Y_BUS"
    export AT_SPI_BUS_ADDRESS="$A11Y_BUS"
else
    say "  !! 没拿到 a11y 总线地址（at-spi2-core 未装或 launcher 起不来），退回会话总线试"
    A11Y_BUS=""
fi
# 之后所有查询都走 a11y 总线（没有就退回会话总线，至少拿到部分信息）。
cbus() {
    if [ -n "$A11Y_BUS" ]; then
        gdbus call --address "$A11Y_BUS" "$@"
    else
        gdbus call --session "$@"
    fi
}

say "== 构建示例 =="
if ! cargo build -q -p kanesumi-harness --features a11y --example atspi_smoke 2>>"$BUS_LOG"; then
    say "构建失败，见上方日志"
    exit 1
fi
BIN="${CARGO_TARGET_DIR:-target}/debug/examples/atspi_smoke"
[ -x "$BIN" ] || { say "找不到示例二进制：$BIN"; exit 1; }

say "== 启动示例（后台，已带 AT_SPI_BUS_ADDRESS）=="
A11Y_SMOKE_SECS="${A11Y_SMOKE_SECS:-20}" "$BIN" >"$APP_STDOUT" 2>&1 &
APP=$!

# 给适配器留出「连总线 + 等激活」的时间。
sleep 8

say "== 注册表 GetChildren（登记在册的应用）=="
CHILDREN=$(cbus --dest org.a11y.atspi.Registry --object-path "$REG_PATH" \
    --method org.a11y.atspi.Accessible.GetChildren 2>&1)
echo "$CHILDREN" | tee -a "$BUS_LOG"

svcs=$(echo "$CHILDREN" | grep -oE "':[0-9]+\.[0-9]+'" | tr -d "'" | sort -u)
if [ -z "$svcs" ]; then
    say "  !! 注册表里没有应用（适配器没连上 a11y 总线，或没被激活）"
fi

say "== 逐个应用读名字 / 角色 / 子元素 =="
for svc in $svcs; do
    name=$(cbus --dest "$svc" --object-path "$REG_PATH" --method org.a11y.atspi.Accessible.GetName 2>/dev/null)
    role=$(cbus --dest "$svc" --object-path "$REG_PATH" --method org.a11y.atspi.Accessible.GetRoleName 2>/dev/null)
    say "  $svc name=$name role=$role"
    kids=$(cbus --dest "$svc" --object-path "$REG_PATH" --method org.a11y.atspi.Accessible.GetChildren 2>/dev/null)
    say "    children=$kids"
    # 顺着子元素读名字：能读到「确定 / 保存（默认）/ 取消」才算树真的过去了。
    for kid in $(echo "$kids" | grep -oE "'/[^']+'" | tr -d "'" | sort -u | head -n 6); do
        kname=$(cbus --dest "$svc" --object-path "$kid" --method org.a11y.atspi.Accessible.GetName 2>/dev/null)
        krole=$(cbus --dest "$svc" --object-path "$kid" --method org.a11y.atspi.Accessible.GetRoleName 2>/dev/null)
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
