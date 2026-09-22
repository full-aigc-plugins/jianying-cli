# 时间线控制面基线

本基线将剪映时间线按钮转换为稳定语义操作。目标是优先通过草稿协议或本地 Runtime 控制，而不是让智能体按屏幕坐标点击。图标外观和坐标不属于公开合同。

## 执行路由

| 路由 | 用途 | 性能与稳定性 | 完成证据 |
|---|---|---|---|
| `draft_protocol` | 可由草稿数据完整表达的编辑 | 最快；无需启动剪映 | 草稿前后差分、引用完整性、代理预览 |
| `runtime_native` | 编辑器会话状态或剪映内部动作 | 较快；要求精确版本 Runtime Profile | 调用前后状态、编辑器事件、草稿/产物变化 |
| `accessibility_adapter` | 暂无底层接口但有稳定可访问性语义的控件 | 中等；受 UI 版本影响 | 控件状态加独立结果验证 |
| `visual_fallback` | 精确版本的最后兜底 | 最慢；不得作为默认生产合同 | 截图、状态与草稿/产物三重证据 |

## 当前已识别控制

机器事实源分成两份：

- `provenance/TIMELINE_CONTROLS.json`：去重后的语义能力目录。
- `provenance/TIMELINE_SURFACE_INVENTORY.json`：目标版本截图中的逐入口清单；同一语义在不同轨道或按钮实例上分别计数。

当前截图和时间线交互面共登记 50 个可操作入口：时间线页签 4、左侧编辑工具栏 12、右侧视图工具栏 12、视图设置弹层 4、轨道头 9、时间线画布手势 9。五个尚未完成子项或语义探测的入口以 `unresolved` 保留，禁止执行。

| 稳定语义 ID | 界面语义 | 首选路由 | 当前 CLI 落点 | 当前状态 |
|---|---|---|---|---|
| `timeline.item.add` | 添加素材或轨道内容 | `draft_protocol` | `media add`、`timeline add-segment` | 部分支持，Job 新增片段仍需补齐 |
| `timeline.tool.select` | 选择工具及其模式 | `runtime_native` | 尚无会话控制命令 | 待实现 |
| `timeline.session.undo` | 撤销 | `runtime_native` | 尚无会话控制命令 | 待实现 |
| `timeline.session.redo` | 重做 | `runtime_native` | 尚无会话控制命令 | 待实现 |
| `timeline.segment.split` | 在播放头或指定时间分割 | `draft_protocol` | `timeline split` | 已支持直接草稿编辑 |
| `timeline.segment.trim_left` | 删除播放头左侧/左裁切 | `draft_protocol` | `timeline trim` / `project cut` | 已有原子能力，需绑定按钮精确语义 |
| `timeline.segment.trim_right` | 删除播放头右侧/右裁切 | `draft_protocol` | `timeline trim` / `project cut` | 已有原子能力，需绑定按钮精确语义 |
| `timeline.segment.delete` | 删除选中片段 | `draft_protocol` | `timeline remove` | 已支持直接草稿编辑 |
| `timeline.marker.toggle` | 添加或移除标记 | `draft_protocol` | 尚无统一 marker 命令 | 待实现 |
| `timeline.ai.smart_rough_cut` | 智能粗剪 | `runtime_native` | 尚无受控状态机 | 待实现 |
| `timeline.ai.script_rough_cut` | 文案粗剪 | `runtime_native` | 尚无受控状态机 | 待实现 |
| `timeline.audio.record` | 录音 | `runtime_native` | 本地音频导入已支持，编辑器录音未支持 | 待实现 |
| `timeline.session.main_track_magnet` | 主轨磁吸 | `runtime_native` | 尚无会话控制命令 | 待运行时确认 |
| `timeline.session.snap` | 时间线自动吸附 | `runtime_native` | 尚无会话控制命令 | 待运行时确认 |
| `timeline.session.linkage` | 视频/音频或片段联动 | `runtime_native` | 草稿引用可验证，会话开关未支持 | 待运行时确认 |
| `timeline.session.preview_axis` | 播放头/预览轴相关开关 | `runtime_native` | 尚无会话控制命令 | 图标语义待探测 |
| `timeline.view.fit` | 时间线适配视图 | `runtime_native` | 不影响草稿 | 待实现 |
| `timeline.view.zoom_out` | 时间线缩小 | `runtime_native` | 不影响草稿 | 待实现 |
| `timeline.view.zoom` | 时间线缩放值 | `runtime_native` | 不影响草稿 | 待实现 |
| `timeline.view.zoom_in` | 时间线放大 | `runtime_native` | 不影响草稿 | 待实现 |
| `timeline.menu.more` | 更多时间线动作 | `runtime_native` | 需要运行时枚举子菜单 | 待探测 |
| `track.lock` | 锁定或解锁轨道 | `draft_protocol` 或 `runtime_native` | 草稿字段需按版本验证 | 待实现 |
| `track.visibility` | 显示或隐藏轨道 | `draft_protocol` 或 `runtime_native` | 草稿字段需按版本验证 | 待实现 |
| `track.audio.mute` | 静音轨道 | `draft_protocol` | `timeline volume` 可表达片段静音 | 部分支持，轨道级语义待补齐 |
| `track.solo` | 独奏轨道 | `runtime_native` | 尚无会话控制命令 | 待实现 |

截图中无法仅凭图标可靠确定的控制保持“待探测”。只有在目标剪映版本中取得可访问性名称、状态变化和独立效果证据后，才能赋予稳定语义 ID 并提升为 supported。

页签菜单、选择工具下拉、联动下拉、预览轴下拉、更多菜单、片段右键和轨道右键属于 7 个可展开表面。
`TIMELINE_SURFACE_INVENTORY.json` 为每个父入口保存 `expansion.kind/status/child_surface_ids/evidence`，
并在 coverage 中分别计数 `expandable_parents=7`、`enumerated_parents=0`。当前“更多时间线设置”从截图记录了
4 个可见子项，但因为尚无可访问性名称仍是 `partial`；其余 6 个入口是 `pending_accessibility`。
父入口存在或截图中看到子项都不代表完整覆盖；只有子项逐个登记、回指父入口且拥有精确版本可访问性名称时，
才能把状态提升为 `enumerated`。

## 直接控制已有覆盖

现有 CLI 已具备不依赖 GUI 的片段移动、批量移动、变速、音量、裁剪、透明度、添加轨道、设置片段、分割、复制、删除、混合模式、智能抠像、色度键、蒙版、背景模糊、音频淡入淡出、滤镜、特效、裁剪矩形、关键帧、转场和图片动画。插件应通过 capability manifest 选择这些命令，禁止退回按钮点击。

## 完成门禁

- 控件存在不等于语义已知；语义未知时不得执行。
- 调用成功不等于效果完成；必须读取会话状态、草稿差分或产物证据。
- Runtime Profile 必须绑定精确产品版本和可执行文件身份。
- GUI fallback 不保存账号秘密，不使用跨版本固定坐标。
- 撤销/重做只控制由本任务拥有的编辑会话；不得影响用户并行编辑历史。
