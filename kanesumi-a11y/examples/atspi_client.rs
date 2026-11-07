// atspi_client —— **像读屏那样消费**无障碍树的客户端（不是读我们自己的结构）。
//
// 为什么需要它：`gdbus` 那套只证明「总线上有这些对象」，而读屏是用 AT-SPI 客户端库走树的。
// 本示例用 `atspi` crate（Odilia 读屏项目那套，和 Orca 的 pyatspi 走同一份 AT-SPI 协议）
// 去连 a11y 总线、列应用、走树、读名字与角色 —— 全程零安装，也不需要声卡。
//
// 用法（Linux，a11y 总线已就绪时）：
//   AT_SPI_BUS_ADDRESS=unix:path=... cargo run -q -p kanesumi-a11y --example atspi_client
// 通常由 scripts/a11y_linux.sh 自动调起（它会先把总线与被测应用准备好）。
//
// 不读 AT_SPI_BUS_ADDRESS 时会退回默认发现路径（问会话总线上的 org.a11y.Bus）。

use std::collections::VecDeque;

use atspi::proxy::accessible::AccessibleProxy;

const REGISTRY: &str = "org.a11y.atspi.Registry";
const ROOT: &str = "/org/a11y/atspi/accessible/root";
const MAX_DEPTH: usize = 4;
const MAX_NODES: usize = 80;

fn bus_address() -> Option<String> {
    if let Ok(addr) = std::env::var("AT_SPI_BUS_ADDRESS") {
        return Some(addr);
    }
    // 退回默认发现：会话总线上问 org.a11y.Bus。
    let out = std::process::Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.a11y.Bus",
            "--object-path",
            "/org/a11y/bus",
            "--method",
            "org.a11y.Bus.GetAddress",
        ])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let start = text.find("unix:")?;
    let rest = &text[start..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

fn main() {
    async_io::block_on(run());
}

async fn run() {
    let Some(addr) = bus_address() else {
        eprintln!("找不到 a11y 总线地址（既没有 AT_SPI_BUS_ADDRESS，也问不到 org.a11y.Bus）");
        std::process::exit(2);
    };
    println!("a11y bus = {addr}");

    let conn = match zbus::connection::Builder::address(addr.as_str()) {
        Ok(builder) => builder.build().await,
        Err(e) => {
            eprintln!("连接地址无效：{e}");
            std::process::exit(2);
        }
    };
    let conn = match conn {
        Ok(c) => c,
        Err(e) => {
            eprintln!("连不上 a11y 总线：{e}");
            std::process::exit(2);
        }
    };

    let registry = match AccessibleProxy::builder(&conn)
        .destination(REGISTRY)
        .and_then(|b| b.path(ROOT))
    {
        Ok(b) => b.build().await,
        Err(e) => {
            eprintln!("注册表对象构造失败：{e}");
            std::process::exit(2);
        }
    };
    let registry = match registry {
        Ok(p) => p,
        Err(e) => {
            eprintln!("注册表不可用：{e}");
            std::process::exit(2);
        }
    };

    let apps = match registry.get_children().await {
        Ok(a) => a,
        Err(e) => {
            eprintln!("读注册表失败：{e}");
            std::process::exit(2);
        }
    };
    println!("登记在册的应用：{} 个", apps.len());

    for app in apps {
        let dest: String = app.name().map(|n| n.to_string()).unwrap_or_default();
        let path: String = app.path().to_string();
        println!("== 应用 {dest} {path} ==");
        walk(&conn, &dest, &path).await;
    }
}

/// 有界广度优先：读每个节点的名字 / 角色名 / 子节点数。
async fn walk(conn: &zbus::Connection, dest: &str, path: &str) {
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    queue.push_back((path.to_string(), 0));
    let mut seen = 0;
    while let Some((node, depth)) = queue.pop_front() {
        if depth > MAX_DEPTH || seen >= MAX_NODES {
            continue;
        }
        seen += 1;
        let Ok(builder) = AccessibleProxy::builder(conn).destination(dest).and_then(|b| b.path(node.as_str()))
        else {
            continue;
        };
        let Ok(proxy) = builder.build().await else { continue };
        let name = proxy.name().await.unwrap_or_default();
        // 用数值角色（`GetRole`）而不是名字（`GetRoleName`）：AccessKit 的 Unix 适配器只实现前者，
        // 而读屏客户端（pyatspi / Orca）本来就是拿数值角色在本地映射名字的。
        let role = match proxy.get_role().await {
            Ok(id) => format!("#{id}"),
            Err(_) => "?".to_string(),
        };
        let kids = proxy.get_children().await.unwrap_or_default();
        println!(
            "  {}{:<10} {:?}  子节点 {}",
            "  ".repeat(depth),
            role,
            name,
            kids.len()
        );
        for k in kids {
            queue.push_back((k.path().to_string(), depth + 1));
        }
    }
}


