// Gallery 骨架 smoke：跨平台运行，输出设计 tokens 一览。
// Wayland+wgpu 外壳 Phase 3 接入（参 Ether-main PLAN.md §4.4 三层测试阶梯）。

use kanesumi_core::MetroTheme;
use kanesumi_gallery::{GalleryPage, page_tree, palette};

fn main() {
    let theme = MetroTheme::ether_dark();
    println!("Kanesumi Gallery — 页树");
    for page in page_tree() {
        println!("  {}: {}", page as usize, page.title());
    }

    println!(
        "\n设计 tokens（{} 主题）",
        GalleryPage::DesignTokens.title()
    );
    for entry in palette(&theme) {
        let (r, g, b) = (
            (entry.color.r * 255.0) as u8,
            (entry.color.g * 255.0) as u8,
            (entry.color.b * 255.0) as u8,
        );
        println!("  #{:<16} #{r:02X}{g:02X}{b:02X}", entry.name);
    }
}
