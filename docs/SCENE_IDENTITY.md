# Scene 命令的节点身份（2026-11-17，Ether 裁定 §263）

消费者：Ether 合成器的 GPU 元件桥。旧 Scene 仍只有命令，不加字段、不改旧接口。

## 新入口

- `Tree::frame_keyed_into(engine, size, dt, out)`：使用同一帧处理与 compose 遍历，输出完整主表面 Scene，并同步记录身份。
- `Tree::scene_command_keys()`：借用最近一次 keyed 出帧的键列表，与 out.commands 一一对应；键包含 Tree 所有者、WidgetId generation、绘制阶段、节点内命令槽。
- `SceneCommandKey`：可复制、比较和哈希的不可构造身份值；`widget_id()` 返回拥有节点。字段私有，避免消费方依赖编码。

键不包含颜色、文本、几何或全场景命令序号。增删兄弟不影响已有节点；同节点改内容只改内容提交，键不变。
paint、paint_after、子裁剪压入/弹出、焦点环、弹层遮罩使用不同阶段，禁止键重复。

## 兼容与失败

旧 frame/frame_into/frame_with_layout 接口不收集身份，不增加稳定帧分配。
keyed 入口临时关闭 damage cull，返回完整 Scene 供保留式 GPU 消费，不把局部命令清单当全场景；damage 信息本身保留。
从旧出帧切到 keyed 时，若没有对应身份旁表，必须重新 compose，不能返回空键配旧命令。
调用旧接口重组场景会使旁表失效；此时查询返回空列表，不能把旧键配新命令。

本阶段仅覆盖主表面。分离弹层、独立图层和滚动器内容仍经既有接口输出，不假装已附带身份；后续使用同一 compose 添加兼容入口。
桥的持久状态由每个输出持有，键必须再加表面/窗口作用域，不能让两个输出共享 GPU 纹理缓存。
