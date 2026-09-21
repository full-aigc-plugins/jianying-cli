## Purpose

定义剪映项目、时间线、轨道、片段、素材和编辑操作的统一强类型契约，使创建、已有草稿编辑、批处理与导出共享同一套可验证语义。

## ADDED Requirements

### Requirement: 强类型项目模型
系统 SHALL 使用可区分的视频、音频、文字、贴纸、滤镜、特效和复合片段模型，并拒绝与片段类型不相容的字段。

#### Scenario: 音频片段携带视觉蒙版
- **WHEN** 输入计划为音频片段提供视觉蒙版
- **THEN** 系统在写入任何草稿文件前返回字段与片段类型不兼容的结构化错误

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

### Requirement: 已有草稿未知字段保留
系统 SHALL 在编辑已有草稿时保留未被目标操作触及的未知非空字段、素材节点和镜像关系。

#### Scenario: 修改文字内容
- **WHEN** 用户只替换一个文字素材的正文
- **THEN** 系统保留该草稿其他未知字段、样式、轨道顺序和未关联素材

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

#### Scenario: 尚未实现原生闭包映射的新增操作
- **WHEN** `edit` Job 包含尚未支持的 `add_material` 或 `add_segment`
- **THEN** 系统返回对应的结构化 `incompatible_capability`，不得创建最终输出或修改源草稿
