#!/usr/bin/env bash
# a11y_linux —— 无障碍（AT-SPI）运行时冒烟：一次跑完，零交互，结果全部落盘。
#
# 判据（两条都要成立才算通过）：
#   1. ~/.local/state/kanesumi/a11y-smoke.log 里有无障碍树，含对话框的三个按钮节点；
#   2. ~/.local/state/kanesumi/a11y-atspi.log 里，AT-SPI 注册表列出我们的应用，
#      且通过 D-Bus 读到的名字 / 角色与树里的一致。
#
# 用法（任何 Linux，无需图形会话）：
#   bash scripts/a11y_linux.sh
#   跨机跑：ssh <host> 'cd <repo> && bash scripts/a11y_linux.sh'
#
# 为什么自己搭一条 a11y 总线，而不用 `org.a11y.Bus`：
#   - 那条总线是**每用户单例**（固定套接字 /run/user/<uid>/at-spi/bus）。桌面会话里已经
#     有一个 launcher 占着它，我们再起一条既绑不上、也可能动到用户正在用的无障碍会话。
#   - GNOME 系默认关无障碍，launcher 被 D-Bus 激活时**只回地址、不起 dbus-daemon**，
#     看起来有地址、套接字却不存在 —— 照着地址查注册表必然 ServiceUnknown。
#   所以：一条一次性的私有总线 + 自己起的 registryd，跑完就收，不碰用户会话。

set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

# 应用侧（accesskit）走 AT_SPI_BUS_ADDRESS；查询侧走 --address。两者都用这条私有总线。
if [ -z "${DBUS_SESSION_BUS_ADDRESS:-}" ]; then
    exec dbus-run-session -- bash "$0" "$@"
fi

export PATH="$HOME/.cargo/bin:$PATH"
OUT="${HOME}/.local/state/kanesumi"
mkdir -p "$OUT" || exit 1
BUS_LOG="$OUT/a11y-atspi.log"
APP_STDOUT="$OUT/a11y-smoke.stdout"
SOCK="$OUT/a11y-bus.sock"
: >"$BUS_LOG"
say() { echo "$*" | tee -a "$BUS_LOG"; }

REG_PATH=/org/a11y/atspi/accessible/root
cleanup() {
    [ -n "${APP:-}" ] && kill "$APP" 2>/dev/null
    [ -n "${REGISTRYD:-}" ] && kill "$REGISTRYD" 2>/dev/null
    [ -n "${BUSPID:-}" ] && kill "$BUSPID" 2>/dev/null
    rm -f "$SOCK"
}
trap cleanup EXIT

say "== 搭私有 a11y 总线 =="
rm -f "$SOCK"
BUSPID=$(dbus-daemon --session --address="unix:path=$SOCK" --fork --print-pid 2>>"$BUS_LOG")
buspid_clean=$(printf "%s" "${BUSPID:-}" | tr -dc '0-9')
BUSPID="$buspid_clean"
if [ -z "$BUSPID" ]; then
    say "  !! dbus-daemon 起不来，见上方日志"
    exit 1
fi
export AT_SPI_BUS_ADDRESS="unix:path=$SOCK"
say "  address=$AT_SPI_BUS_ADDRESS pid=$BUSPID"

say "== 起 AT-SPI 注册表（at-spi2-registryd）=="
REG_LOG="$OUT/a11y-registryd.log"
/usr/libexec/at-spi2-registryd >"$REG_LOG" 2>&1 &
REGISTRYD=$!
sleep 2
if grep -q "well-known name" "$REG_LOG" 2>/dev/null; then
    say "  $(head -n 1 "$REG_LOG")"
else
    say "  !! 注册表没起来，见 $REG_LOG"
fi

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
CHILDREN=$(gdbus call --address "$AT_SPI_BUS_ADDRESS" --dest org.a11y.atspi.Registry \
    --object-path "$REG_PATH" --method org.a11y.atspi.Accessible.GetChildren 2>&1)
echo "$CHILDREN" | tee -a "$BUS_LOG"

svcs=$(echo "$CHILDREN" | grep -oE "':[0-9]+\.[0-9]+'" | tr -d "'" | sort -u)
[ -z "$svcs" ] && say "  !! 注册表里没有应用（适配器没连上，或没被激活）"

say "== 逐个应用读名字 / 角色 / 子元素 =="
for svc in $svcs; do
    name=$(gdbus call --address "$AT_SPI_BUS_ADDRESS" --dest "$svc" --object-path "$REG_PATH" \
        --method org.a11y.atspi.Accessible.GetName 2>/dev/null)
    role=$(gdbus call --address "$AT_SPI_BUS_ADDRESS" --dest "$svc" --object-path "$REG_PATH" \
        --method org.a11y.atspi.Accessible.GetRoleName 2>/dev/null)
    say "  $svc name=$name role=$role"
    kids=$(gdbus call --address "$AT_SPI_BUS_ADDRESS" --dest "$svc" --object-path "$REG_PATH" \
        --method org.a11y.atspi.Accessible.GetChildren 2>/dev/null)
    say "    children=$kids"
    # 顺子元素读名字：能读到「确定 / 保存（默认）/ 取消」才算树真的过去了。
    for kid in $(echo "$kids" | grep -oE "'/[^']+'" | tr -d "'" | sort -u | head -n 6); do
        kname=$(gdbus call --address "$AT_SPI_BUS_ADDRESS" --dest "$svc" --object-path "$kid" \
            --method org.a11y.atspi.Accessible.GetName 2>/dev/null)
        krole=$(gdbus call --address "$AT_SPI_BUS_ADDRESS" --dest "$svc" --object-path "$kid" \
            --method org.a11y.atspi.Accessible.GetRoleName 2>/dev/null)
        say "      $kid name=$kname role=$krole"
    done
done

wait "$APP"
rc=$?
APP=""
say "== 示例退出 rc=$rc，产物：=="
say "  树摘要  $OUT/a11y-smoke.log"
say "  stdout  $APP_STDOUT"
say "  总线侧  $BUS_LOG"
exit $rc
