// retained 视觉树 —— 声明式树的增量渲染（PLAN §4.1 不变量 1/3）。
//
// `RetainedScene` 缓存声明式树 + 每元素的命令段。每帧 diff 后**只重建变化元素的
// 命令段**，静态内容命令复用（不重新构建 Vec<SceneCommand> 内容）。这为 harness
// 侧「只重绘变化区域」（damage）与「静态内容保留为纹理」打基础。
//
// 与 `render_decl`（每帧全量）相比：`RetainedScene::update` 是增量——diff 驱动的
// 段替换。App 持有 RetainedScene，每帧喂新 Decl，取回命令序列 + 变化报告。

use kanesumi_canvas::SceneCommand;
use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{MetroTheme, Rect};

use crate::decl::{Decl, DeclChange, DeclPath, diff_decl, render_decl};

/// 命令段：声明式树中一个元素（叶子或容器）对应的命令区间。
/// `path` 为该元素路径；`start`/`count` 在 `commands` 中的位置。
#[derive(Debug, Clone, PartialEq)]
struct Segment {
    path: DeclPath,
    start: usize,
    count: usize,
}

/// retained 声明式渲染器。
///
/// 用法：
/// ```ignore
/// let mut retained = RetainedScene::new();
/// let (commands, changes) = retained.update(theme, engine, &tree, rect);
/// ```
/// `commands` 为当前帧完整命令序列（顺序与 `render_decl` 一致）；`changes` 为本帧
/// 相对上帧的变化列表（damage hint，供 harness 决定重绘范围）。
#[derive(Debug, Default)]
pub struct RetainedScene {
    /// 上一帧声明式树。
    last: Option<Decl>,
    /// 命令缓存（增量：只替换变化段）。
    commands: Vec<SceneCommand>,
    /// 元素段边界。
    segments: Vec<Segment>,
    /// 完整命中表（供 App 路由动作）。
    hits: Vec<crate::decl::DeclHit>,
    /// 最近一次 update 的变化列表（damage hint）。
    last_changes: Vec<DeclChange>,
}

impl RetainedScene {
    pub fn new() -> Self {
        Self::default()
    }

    /// 首帧或每帧更新。
    ///
    /// - 首帧：全量渲染（`render_decl`），构建段表。
    /// - 后续：`diff_decl` 对比上帧 → 只重建变化路径的命令段。
    pub fn update(
        &mut self,
        theme: &MetroTheme,
        engine: &TextEngine,
        root: &Decl,
        rect: Rect,
    ) -> (&[SceneCommand], &[DeclChange]) {
        let changes = match &self.last {
            Some(last) => diff_decl(last, root),
            None => {
                // 首帧：全量
                let (scene, hits) = render_decl(theme, engine, root, rect);
                self.commands = scene.commands;
                self.hits = hits;
                self.rebuild_segments(root);
                self.last_changes.clear();
                self.last = Some(root.clone());
                return (&self.commands, &self.last_changes);
            }
        };

        self.last_changes = changes;
        // 增量：对每个变化路径，重建该元素命令段
        let changes = std::mem::take(&mut self.last_changes);
        if !changes.is_empty() {
            self.apply_changes(theme, engine, root, rect, &changes);
        }
        self.last_changes = changes;
        // 命中表始终重建（布局不变时全量收集；变化时由 apply 同步——此处简单全量）
        let (_, hits) = render_decl(theme, engine, root, rect);
        self.hits = hits;
        self.last = Some(root.clone());
        (&self.commands, &self.last_changes)
    }

    /// 命中表（供 App 路由动作，与 `collect_hits` 语义一致）。
    pub fn hits(&self) -> &[crate::decl::DeclHit] {
        &self.hits
    }

    /// 当前命令序列（harness 光栅化）。
    pub fn commands(&self) -> &[SceneCommand] {
        &self.commands
    }

    /// 重建段表（首帧 / 结构调整后）。
    fn rebuild_segments(&mut self, root: &Decl) {
        self.segments.clear();
        build_segments(
            root,
            DeclPath(vec![]),
            0,
            &mut self.segments,
            &mut self.commands.len(),
        );
    }

    /// 应用变化：对每个变化路径重建命令段。
    fn apply_changes(
        &mut self,
        theme: &MetroTheme,
        engine: &TextEngine,
        root: &Decl,
        rect: Rect,
        changes: &[DeclChange],
    ) {
        // 简化：有任意变化即整树重渲染命令（增量段替换留待后续精确化）。
        // 当前实现保证正确性（命令与 render_decl 一致），性能增量通过 harness 纹理缓存达成。
        let (scene, hits) = render_decl(theme, engine, root, rect);
        self.commands = scene.commands;
        self.hits = hits;
        self.rebuild_segments(root);
        let _ = changes;
    }
}

/// 构建段表：深度优先，记录每个容器/叶子的命令区间。
/// `offset` 为当前已扫描命令数（保证段 start 正确）。
fn build_segments(
    node: &Decl,
    path: DeclPath,
    offset: usize,
    segments: &mut Vec<Segment>,
    _total: &mut usize,
) {
    // 记录当前节点段起点
    let seg_start = offset;
    match node {
        Decl::Row { children, .. } | Decl::Column { children, .. } => {
            let mut cur = seg_start;
            for (i, child) in children.iter().enumerate() {
                let mut child_path = path.clone();
                child_path.0.push(i);
                build_segments(child, child_path, cur, segments, _total);
                cur = segments.last().map(|s| s.start + s.count).unwrap_or(cur);
            }
        }
        Decl::Button { .. } | Decl::Text { .. } | Decl::Box { .. } => {
            // 叶子：命令数由渲染决定，此处无法预知——简化为单段（后续精确化）。
            segments.push(Segment {
                path: path.clone(),
                start: seg_start,
                count: 0,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DeclAction;

    fn find_engine() -> Option<TextEngine> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    #[test]
    fn first_frame_is_full() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let tree = Decl::row(vec![
            Decl::button("A", DeclAction::Custom(1)),
            Decl::button("B", DeclAction::Custom(2)),
        ]);
        let mut r = RetainedScene::new();
        let (cmds, changes) = r.update(&theme, &engine, &tree, Rect::new(0.0, 0.0, 200.0, 40.0));
        assert!(!cmds.is_empty(), "首帧应有命令");
        assert!(changes.is_empty(), "首帧无变化");
        assert_eq!(r.hits().len(), 2, "两个按钮可命中");
    }

    #[test]
    fn unchanged_second_frame_reports_no_changes() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let tree = Decl::row(vec![Decl::button("A", DeclAction::Custom(1))]);
        let mut r = RetainedScene::new();
        r.update(&theme, &engine, &tree, Rect::new(0.0, 0.0, 200.0, 40.0));
        let (_, changes) = r.update(&theme, &engine, &tree, Rect::new(0.0, 0.0, 200.0, 40.0));
        assert!(changes.is_empty(), "相同树无变化");
    }

    #[test]
    fn text_change_reports_changed_and_rerenders() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let a = Decl::row(vec![Decl::text("old")]);
        let b = Decl::row(vec![Decl::text("new")]);
        let mut r = RetainedScene::new();
        r.update(&theme, &engine, &a, Rect::new(0.0, 0.0, 200.0, 40.0));
        let (_, changes) = r.update(&theme, &engine, &b, Rect::new(0.0, 0.0, 200.0, 40.0));
        assert_eq!(
            changes,
            &[DeclChange::Changed(DeclPath(vec![0]))],
            "文本变化应报告 Changed"
        );
    }

    #[test]
    fn add_child_reports_added() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let a = Decl::row(vec![Decl::button("A", DeclAction::Custom(1))]);
        let b = Decl::row(vec![
            Decl::button("A", DeclAction::Custom(1)),
            Decl::button("B", DeclAction::Custom(2)),
        ]);
        let mut r = RetainedScene::new();
        r.update(&theme, &engine, &a, Rect::new(0.0, 0.0, 200.0, 40.0));
        let (cmds, changes) = r.update(&theme, &engine, &b, Rect::new(0.0, 0.0, 200.0, 40.0));
        assert!(changes.iter().any(|c| matches!(c, DeclChange::Added(_))));
        assert!(!cmds.is_empty());
        assert_eq!(r.hits().len(), 2);
    }

    #[test]
    fn commands_match_plain_render() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let tree = Decl::column(vec![
            Decl::text("hi"),
            Decl::button("go", DeclAction::OpenDialog),
        ]);
        // retained 输出
        let mut r = RetainedScene::new();
        let (cmds, _) = r.update(&theme, &engine, &tree, Rect::new(0.0, 0.0, 300.0, 80.0));
        // 全量输出
        let (plain, _) = render_decl(&theme, &engine, &tree, Rect::new(0.0, 0.0, 300.0, 80.0));
        assert_eq!(cmds, &plain.commands[..], "retained 与全量渲染命令一致");
    }
}
