#!/usr/bin/env bash
# a11y_linux —— 无障碍（AT-SPI）运行时验收：一次跑完，零交互，结果全部落盘。
#
# 判据：~/.local/state/kanesumi/a11y-atspi.log 里，**通过 AT-SPI 的 D-Bus 接口**读到的
# 节点名字包含界面上的那几个标签（确定 / 保存（默认）/ 取消 / 名称 / 无障碍冒烟），
# 并且 ~/.local/state/kanesumi/a11y-smoke.log 里我们自己那份树与之对应。
#
# 用法（任何 Linux，无需图形会话）：
#   bash scripts/a11y_linux.sh
#   跨机跑：ssh <host> 'cd <repo> && bash scripts/a11y_linux.sh'
#
# 三个必须知道的坑（都是本脚本踩出来的）：
#   1. **a11y 总线是每用户单例**（/run/user/<uid>/at-spi/bus）。桌面会话里已经有人占着，
#      我们再起既绑不上、也可能动到用户正在用的无障碍会话 → 用私有 XDG_RUNTIME_DIR 隔离。
#   2. **先用 --a11y=1 起 launcher，再问地址**。若先问 `org.a11y.Bus`，D-Bus 会把 launcher
#      激活成「无障碍关闭」的桩：只回地址、不起 dbus-daemon，地址有、套接字无。
#   3. accesskit 的 Linux 适配器只在**会话总线**上看到 `org.a11y.Status.IsEnabled == true`
#      才连接。这个属性由 launcher 在我们这条一次性会话总线上提供（--a11y=1），
#      所以 launcher 必须与 app 在同一个 dbus-run-session 里。

set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

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

# 私有运行时目录：a11y 总线套接字落在里面，与用户会话彻底隔离。
export XDG_RUNTIME_DIR="$OUT/xdg-runtime"
rm -rf "$XDG_RUNTIME_DIR"
mkdir -p -m 700 "$XDG_RUNTIME_DIR"

REG_PATH=/org/a11y/atspi/accessible/root
cleanup() {
    [ -n "${APP:-}" ] && kill "$APP" 2>/dev/null
    [ -n "${LAUNCHER:-}" ] && kill "$LAUNCHER" 2>/dev/null
    pkill -f "xdg-runtime/at-spi" 2>/dev/null
    rm -rf "$XDG_RUNTIME_DIR"
}
trap cleanup EXIT

say "== 起 a11y 总线（at-spi-bus-launcher --a11y=1，私有 XDG）=="
SOCK="$XDG_RUNTIME_DIR/at-spi/bus"
/usr/libexec/at-spi-bus-launcher --a11y=1 --launch-immediately >>"$BUS_LOG" 2>&1 &
LAUNCHER=$!
# 等套接字出现再问地址。**先问地址会让 D-Bus 去激活一个 launcher**，而激活出来的进程继承
# 的是 dbus-daemon 的环境 —— dbus-daemon 由 dbus-run-session 在本脚本设置 XDG 之前启动，
# 于是那个 launcher 是「无障碍关闭 + 全局套接字」，症状是地址指向 /run/user/<uid>/at-spi。
for _ in $(seq 1 25); do
    [ -S "$SOCK" ] && break
    sleep 0.2
done
ADDR=$(gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
    --method org.a11y.Bus.GetAddress 2>&1 | sed -n "s/^('\(.*\)',)$/\1/p")
export AT_SPI_BUS_ADDRESS="$ADDR"
say "  address=$ADDR"
say "  IsEnabled=$(gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus \
    --method org.freedesktop.DBus.Properties.Get org.a11y.Status IsEnabled 2>&1)"
if [ ! -S "$SOCK" ]; then
    say "  !! 私有 a11y 总线套接字没出现（$SOCK）"
    say "     XDG_RUNTIME_DIR=$XDG_RUNTIME_DIR perms=$(stat -c %a "$XDG_RUNTIME_DIR" 2>/dev/null)"
    say "     launcher alive=$(kill -0 "$LAUNCHER" 2>/dev/null && echo yes || echo no)"
    say "     见 $BUS_LOG"
    exit 2
fi
say "  socket=$SOCK 就绪"

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
sleep 8   # 给适配器留出「连总线 + 等激活」的时间

q() { gdbus call --address "$AT_SPI_BUS_ADDRESS" --dest "$1" --object-path "$2" --method "$3" 2>&1; }
prop() { gdbus call --address "$AT_SPI_BUS_ADDRESS" --dest "$1" --object-path "$2" \
    --method org.freedesktop.DBus.Properties.Get org.a11y.atspi.Accessible "$3" 2>&1; }

APPS=$(q org.a11y.atspi.Registry "$REG_PATH" org.a11y.atspi.Accessible.GetChildren)
say "== 注册表 GetChildren =="
say "  $APPS"

FOUND="$OUT/a11y-atspi-names.txt"
: >"$FOUND"
for svc in $(echo "$APPS" | grep -oE "':[0-9]+\.[0-9]+'" | tr -d "'" | sort -u); do
    say "== 应用 $svc =="
    say "  root children=$(q "$svc" "$REG_PATH" org.a11y.atspi.Accessible.GetChildren)"
    # 广度优先走两三层就够：本界面的标签都在前两层。
    L1=$(q "$svc" "$REG_PATH" org.a11y.atspi.Accessible.GetChildren | grep -oE "'/[^']+'" | tr -d "'" | sort -u)
    for n1 in $L1; do
        name=$(prop "$svc" "$n1" Name)
        role=$(q "$svc" "$n1" org.a11y.atspi.Accessible.GetRole)
        say "    $n1 name=$name role=$role"
        echo "$name" >>"$FOUND"
        for n2 in $(q "$svc" "$n1" org.a11y.atspi.Accessible.GetChildren | grep -oE "'/[^']+'" | tr -d "'" | sort -u); do
            nm2=$(prop "$svc" "$n2" Name)
            rl2=$(q "$svc" "$n2" org.a11y.atspi.Accessible.GetRole)
            say "      $n2 name=$nm2 role=$rl2"
            echo "$nm2" >>"$FOUND"
        done
    done
done

say "== 判定 =="
missing=""
for want in 无障碍冒烟 名称 确定 保存（默认） 取消; do
    if grep -q "$want" "$FOUND"; then
        say "  ✔ $want"
    else
        say "  ✘ $want"
        missing="$missing $want"
    fi
done
if [ -z "$missing" ]; then
    say "结论：通过 —— AT-SPI 侧读到的名字与界面标签一致。"
else
    say "结论：未通过 —— 缺:$missing"
fi

wait "$APP"
say "== 示例退出 rc=$?，产物 =="
say "  树摘要  $OUT/a11y-smoke.log"
say "  总线侧  $BUS_LOG"
[ -z "$missing" ] || exit 2
