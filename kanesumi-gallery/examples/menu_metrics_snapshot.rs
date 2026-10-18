// 菜单宽度回归快照：只用元素树布局，在任意平台运行，不连接桌面会话。
// 参 docs/CONTROL_SPEC.md §8.5。
use kanesumi_controls::menu_bar::{MenuBarItem, MetroMenuBar};
use kanesumi_core::{MetroTheme, Size, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Label, Stack};
use kanesumi_harness::element::{Action, Align, Insets, LayoutProps, Tree, WidgetId};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp, TreeHost};

struct MenuPage {
    config: AppConfig,
    theme: MetroTheme,
}

impl TreeApp for MenuPage {
    fn config(&self) -> &AppConfig {
        &self.config
    }
    fn theme(&self) -> MetroTheme {
        self.theme
    }
    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, _action: Action) {}

    fn build(&mut self, tree: &mut Tree) {
        let page = tree.insert(
            tree.root(),
            Border::new()
                .background(ThemeColor::Background)
                .padding(Insets::all(16.0)),
        );
        let column = tree.insert(page, Stack::column().with_spacing(12.0));
        tree.insert(
            column,
            Label::new("菜单自然宽度 · 中英文标题").style(self.theme.typography.title),
        );
        tree.insert(
            column,
            Label::new("文字保持完整；最后一项 Help / 帮助可见。")
                .color(ThemeColor::OnSurfaceVariant),
        );
        for (caption, labels) in [
            ("英文", vec!["File", "Edit", "View", "Help"]),
            ("中文", vec!["文件", "编辑", "查看", "帮助"]),
        ] {
            let row = tree.insert(column, Stack::row());
            tree.insert_with(
                row,
                Label::new(caption),
                LayoutProps {
                    width: Some(83.3),
                    v_align: Align::Center,
                    ..LayoutProps::default()
                },
            );
            let mut bar = MetroMenuBar::new(
                labels
                    .into_iter()
                    .map(|label| MenuBarItem::new(label, vec![]))
                    .collect(),
            );
            bar.header_height = 30.0;
            tree.insert_with(
                row,
                bar,
                LayoutProps {
                    v_align: Align::Center,
                    ..LayoutProps::default()
                },
            );
        }
    }
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let output = args.next().ok_or("请指定 PNG 输出路径")?;
    let dark = args.next().as_deref() == Some("dark");
    let app = MenuPage {
        config: AppConfig::new(
            "org.ether.kanesumi.menumetrics",
            "菜单宽度",
            EtherRole::Browser,
            720.0,
            210.0,
        ),
        theme: if dark {
            MetroTheme::ether_dark()
        } else {
            MetroTheme::ether_light()
        },
    };
    let engine = kanesumi_harness::element::testing::test_engine();
    kanesumi_harness::snapshot::render_png(
        &mut TreeHost::new(app),
        &engine,
        Size::new(720.0, 210.0),
        2.0,
        1,
        std::path::Path::new(&output),
    )?;
    Ok(())
}
