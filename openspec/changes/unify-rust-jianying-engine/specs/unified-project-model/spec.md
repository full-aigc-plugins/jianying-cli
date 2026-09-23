## Purpose

定义剪映项目、时间线、轨道、片段、素材和编辑操作的统一强类型契约，使创建、已有草稿编辑、批处理与导出共享同一套可验证语义。

## ADDED Requirements

### Requirement: 强类型项目模型
系统 SHALL 使用可区分的视频、音频、文字、贴纸、滤镜、特效和复合片段模型，并拒绝与片段类型不相容的字段。

系统 SHALL 将 clip、crop、transform、关键帧、蒙版、色度、背景、混合、动画、转场、音频效果与文字样式表达为可独立校验的领域值对象；兼容输入的投影不得丢失这些语义，也不得以原始 compatibility payload 代替统一领域模型。

#### Scenario: 音频片段携带视觉蒙版
- **WHEN** 输入计划为音频片段提供视觉蒙版
- **THEN** 系统在写入任何草稿文件前返回字段与片段类型不兼容的结构化错误

#### Scenario: v1 视觉与 clip 语义进入统一领域
- **WHEN** 合法 v1 视频片段声明速度、音量、同步变调、八点 crop 和位置缩放旋转透明度
- **THEN** v1 转换器必须把这些字段投影为强类型领域值对象，并能在不依赖 compatibility payload 的情况下恢复等价创建语义

#### Scenario: v1 高级片段语义完成双向投影
- **WHEN** 合法 v1 计划声明关键帧、蒙版、色度抠图、背景、混合、动画、转场、淡入淡出、音频效果或文字样式
- **THEN** v1 转换器必须将字段投影为与片段类型匹配的独立领域值对象，并由 Domain-to-Plan 在不读取 compatibility payload 的情况下恢复字段级等价语义

#### Scenario: parity 完成声明与矩阵冲突
- **WHEN** pyJianYingDraft 的 function 或 data structure 仍存在 `partial`
- **THEN** parity 矩阵必须声明整体 incomplete、列出明确 gap 与未勾选 blocking task；来源门禁拒绝完整声明或已勾选的阻塞任务

### Requirement: 版本化作业协议
系统 SHALL 提供版本化统一 Job/Plan Schema，至少表达创建、编辑、检查、发布、导出和批处理，并保持 `jianying-cli-plan/v1` 的显式兼容转换。

#### Scenario: 旧计划继续构建
- **WHEN** 用户提交合法的 `jianying-cli-plan/v1`
- **THEN** 系统将其转换为统一领域模型并产生与兼容基线等价的草稿

### Requirement: 时间与帧网格不变量
系统 MUST 使用整数微秒保存公共时间语义，并在需要原生帧对齐时记录量化结果，不得静默删除、重叠或延长受保护内容。

#### Scenario: 时间无法安全量化
- **WHEN** 一个编辑操作无法在目标帧率下保持声明的内容边界
- **THEN** 系统拒绝该操作并报告原始时间、目标帧网格和冲突范围

#### Scenario: 代理渲染遵守源片段半开区间
- **WHEN** Job 从四秒素材选择 `[1s,3s)` 并生成两秒目标片段
- **THEN** 草稿和代理预览均只包含所选两秒，不能绕过 `source_timerange` 输出完整源文件

#### Scenario: 代理帧率遵守草稿帧网格
- **WHEN** `jianying-job/v2` 声明非 30 fps 的合法项目帧率并生成代理
- **THEN** 代理视频流帧率与草稿声明一致，不能被渲染器固定改为 30 fps；缺失、非有限或超出可执行范围的草稿帧率必须失败关闭

#### Scenario: 代理渲染还原素材裁剪
- **WHEN** 视频素材的归一化 crop 只保留源画面的右半部分
- **THEN** 代理预览在画布缩放与补边前先应用该 crop，不能重新暴露被裁掉的左半部分

#### Scenario: 代理渲染还原片段缩放
- **WHEN** `jianying-job/v2` 把竖屏视频片段放入横屏画布，并声明足以填满画布的片段缩放
- **THEN** `render proxy` 必须应用草稿 `clip.scale`，独立抽帧的画布边缘不得仍是未缩放时的黑边；无效或非有限的缩放必须失败关闭
- **AND** 该代理能力不代表背景填充、转场或原生导出已得到验收

#### Scenario: 代理预览烧录中文文字
- **WHEN** `render proxy --burn-captions` 遇到中文文字片段
- **THEN** 系统使用本机可用的 CJK 字体渲染可区分的真实字形；若找不到兼容字体则明确失败，不能静默输出相同的缺字方框

### Requirement: 已有草稿未知字段保留
系统 SHALL 在编辑已有草稿时保留未被目标操作触及的未知非空字段、素材节点和镜像关系。

#### Scenario: 修改文字内容
- **WHEN** 用户只替换一个文字素材的正文
- **THEN** 系统保留该草稿其他未知字段、样式、轨道顺序和未关联素材

#### Scenario: 替换文字后重算样式区间
- **WHEN** 模板文字长度发生变化且调用方未关闭样式重算
- **THEN** 系统按 UTF-16 码元比例向上取整重算每个非空样式区间，保留区间之外的未知样式字段；该 Unicode 安全差异必须在固定 Python/Rust 差分中明确记录

### Requirement: 模板素材替换与轨道导入闭包
系统 SHALL 按声明顺序执行短素材裁头、裁尾、裁尾并对齐后续片段或居中收缩，以及长素材向前延长、向后延长、推动后续片段或截断素材尾部策略。

按片段替换时系统 MUST 创建独立素材身份，不得改变仍被其他片段共享的原素材；替换失败 MUST 保持源草稿及其素材目录不变。

导入轨道时系统 SHALL 为轨道、片段及递归素材引用闭包生成新 ID、重写 JSON 对象和 JSON 字符串内的已知引用、复制本地素材，并在删除源草稿后保持目标草稿独立有效；不含已知引用的未知 JSON 字符串不得被重新序列化。

#### Scenario: 短素材裁尾并对齐后续片段
- **WHEN** 两秒片段替换为一秒素材并选择 `cut_tail_align`
- **THEN** 目标片段缩短一秒且同轨后续片段整体前移一秒，其他共享原素材的片段保持原素材身份

#### Scenario: 长素材策略按声明顺序回退
- **WHEN** 首选延长策略会与相邻片段冲突且后续声明 `cut_material_tail`
- **THEN** 系统选择首个可执行策略，并把素材源区间截断为原目标片段时长

#### Scenario: 递归导入素材引用闭包
- **WHEN** 被导入轨道的主素材通过普通字段或 JSON 字符串继续引用文字和本地视频素材
- **THEN** 系统递归复制全部引用节点并重映射 ID；删除源草稿后目标草稿仍通过镜像、注册和本地文件完整性校验

### Requirement: Job v2 隔离编辑执行
系统 SHALL 仅在 `edit` Job 携带至少一个强类型 `EditOperation` 时执行已有草稿编辑，并 MUST 将全部操作应用到全新输出副本；源草稿在成功、失败和不支持的操作路径上均保持逐文件不变。

#### Scenario: 应用保存后的元数据不是可读 JSON 对象
- **WHEN** `draft_meta_info.json` 为不透明编码、损坏 JSON 或非对象 JSON
- **THEN** `job run edit` 在创建工作副本前返回 `unsupported_draft_encoding`，保留 task ID 和文件名，不回显文件内容、不建议无条件重试、不覆盖源草稿未知元数据

#### Scenario: 在隔离副本中组合编辑
- **WHEN** `jianying-job/v2` 对已有草稿提交 `replace_text`、`move_segment` 或 `remove_segment` 操作
- **THEN** `job run --json` 原子产生可验证的新草稿，返回逐操作结果和源目录不变证据，且输出草稿使用最终输出路径作为身份

#### Scenario: edit 没有操作
- **WHEN** `edit` Job 的 `operations` 缺失或为空
- **THEN** 系统在复制或写入任何草稿前返回 `invalid_job`

#### Scenario: 新增完整强类型片段与轨道操作
- **WHEN** `edit` Job 依次声明本地素材、新增轨道、新增携带关键帧、蒙版、色度、背景、混合、动画、转场和淡入淡出的片段，再重排或删除轨道
- **THEN** 系统仅通过统一领域模型和共享 Domain-to-Wire 编译器生成完整引用闭包，保留声明的轨道与片段 ID，并在隔离副本内原子提交

#### Scenario: 新增片段引用未声明素材
- **WHEN** `add_segment` 引用本 Job 未先声明的本地素材，或者 `add_material` 在 Job 结束时未被任何片段消费
- **THEN** 系统返回包含具体原因的 `invalid_job`，删除 staging 和内部临时草稿，不创建最终输出且源草稿逐文件不变
