# Scene 身份导出原型（2026-11-17，裁定 §263）

- 分支：codex/scene-identity；基线 e830107。旧 Scene 结构、旧公开方法签名保持不变。
- 新入口 frame_keyed_into 和 scene_command_keys 共享原 compose 遍历，不复制另一份裁剪/图层逻辑。
- 身份包含 Tree 所有者、WidgetId generation、绘制阶段和节点内槽位；不取颜色、文本、几何或全局命令下标。
- keyed 出帧关闭 damage 剔除以交付完整 GPU 列表，帧后恢复原设置；旧入口不收集身份。
- 12 条专项回归覆盖旧场景相等、兄弟增删、arena 复用、跨树区分、内容变化、裁剪/after/focus、damage、旧入口失效、稳态存储复用、图层与弹层遮罩。
- 完整工作区：2066 passed / 0 failed / 5 个既有 ignored；Clippy 退出 0；API 相对 e830107 只增 4 行、移除 0 行。
- 这是原型，尚未合到主线、未更新 Ether gitlink。GPU 桥和 SSD/通知消费方尚未实现，不能把本件记作供应门判据 2 通过。
- 旧入口的绘制 Scene 已逐命令相等；本件没有改变控件视觉，没有重新生成产品快照。
- 后续先在合成器录 C0，再落输出持有的桥状态和 GPU 映射；已有 WinDraw 只持 GLES 纹理，Scene Image 持 CPU RGBA，必须先查清资产所有权，不能捏造空 Image 或绕回 CPU 整幅渲染。
- 日志：~/.local/state/scene-identity-tests-20261117.log、scene-identity-workspace-20261117.log、scene-identity-clippy-20261117.log。
