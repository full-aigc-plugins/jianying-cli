## Purpose

定义剪映基础版与专业版权益、官方素材授权收据和能力解析的可观察契约，确保工作流只在证据充分时使用受限内容。

## ADDED Requirements

### Requirement: Installation identity and account edition are independent
系统 SHALL 分别报告应用产品/版本与账号版型；不得依据应用名称、bundle ID、可见菜单或安装包版本推断账号具有专业版权益。

#### Scenario: Professional application name without account evidence
- **WHEN** 系统只观察到名称为“剪映专业版”的安装包而没有有效账号权益证据
- **THEN** 账号版型为 `unknown`，专业版能力不可自动路由

#### Scenario: Verified professional entitlement
- **WHEN** 可验证证据表明专业版权益处于有效期内
- **THEN** 系统报告 `professional` 和证据有效期，并允许解析明确要求专业版的能力

### Requirement: Entitlement evidence is explicit and expiring
权益证据 MUST 记录证据来源、观察时间、状态和可选失效时间，且 MUST 排除 Cookie、Token、手机号、用户名和其他账号秘密。

#### Scenario: Expired entitlement
- **WHEN** 专业版权益证据的失效时间早于当前校验时间
- **THEN** 系统以结构化错误拒绝专业能力，不得自动降级为基础版执行同一 Job

#### Scenario: Evidence contains secrets
- **WHEN** 权益证据包含认证秘密或原始账号标识
- **THEN** 系统拒绝导入且不写入持久化状态

### Requirement: Official resources require typed immutable receipts
每个进入草稿的剪映官方资源 MUST 具有带类型的不可变收据。系统 SHALL 区分视频/图片、音乐、文字模板、贴纸、画面特效、转场、字幕样式及未来未知类型，并至少绑定资源标识、来源、单项权益标记、内容身份、取得时间、条款引用和用途限制。

#### Scenario: Local file matches receipt
- **WHEN** 官方素材文件的字节长度和 SHA-256 与收据一致且当前权益满足版型要求
- **THEN** 系统返回可使用的素材证据和解析后的用途约束

#### Scenario: Downloaded file changed
- **WHEN** 本地素材内容与收据摘要不一致
- **THEN** 系统拒绝素材并报告 `asset_identity_mismatch`

#### Scenario: Preview-only or unknown license
- **WHEN** 素材只可预览、条款引用缺失或用途范围未知
- **THEN** 系统不得将素材编译进可交付草稿

#### Scenario: Resource family is unknown
- **WHEN** GUI 出现 CLI 尚未建模的新资源类型
- **THEN** 系统保留原始类型标识并停止自动应用，不得把它误归类为普通视频素材

#### Scenario: Transition has no adjacent clips
- **WHEN** 工作流选择转场但目标时间线没有满足要求的相邻片段
- **THEN** 系统在取得或应用资源前返回前置条件失败，不把 GUI 提示当成下载失败

#### Scenario: Music has no explicit delivery scope
- **WHEN** 音乐资源可下载但收据没有覆盖目标发布渠道和商业用途
- **THEN** 系统允许保存候选证据但禁止编译进可交付成片

### Requirement: Basic and professional routing is fail closed
工作流 SHALL 声明所需版型和逐资源类型能力；基础版只能使用基础、仍在有效期内的限免或本地已授权资源，专业版能力只有在有效专业权益与单项资源收据同时满足时可用。菱形、限免、下载按钮或 GUI 可见性只构成观察证据，不单独构成授权结论。

#### Scenario: Basic workflow uses local media
- **WHEN** Job 仅使用本地已授权素材且不要求专业能力
- **THEN** 系统允许基础版或专业版运行时执行

#### Scenario: Professional asset requested by basic edition
- **WHEN** Job 请求专业素材而当前有效版型为基础版
- **THEN** 系统返回包含所需版型和缺失能力的结构化失败，不替换为相似素材

### Requirement: GUI acquisition and Rust verification remain separated
GUI Adapter MAY 搜索、预览和下载用户账号可访问的官方素材，但 Rust CLI MUST 独立校验权益证据、素材身份和用途约束后才能将其提供给草稿编译器。

#### Scenario: GUI reports download success without a receipt
- **WHEN** GUI Adapter 声称下载成功但没有提交完整素材收据
- **THEN** Rust CLI 拒绝登记该素材

#### Scenario: Verified acquisition handoff
- **WHEN** GUI Adapter 提交完整收据且本地文件通过身份与权益校验
- **THEN** Rust CLI 输出不含账号秘密的机器可读验证结果

### Requirement: Real-editor acceptance evidence is replayable and content-bound
系统 SHALL 通过 Rust CLI 独立复核真实剪映验收记录。原生导出证据 MUST 绑定精确编辑器身份、
发布 CLI 身份、草稿摘要、明确批准、来源清单、冷重开/连续播放/导出观察、原生输出摘要及媒体流；
人工审查状态 MUST 独立报告，不得由导出成功自动推断。

#### Scenario: Complete native-export evidence is replayed
- **WHEN** 验收记录包含全部身份与观察绑定，且原生输出、联系表、来源清单和 `ffprobe` 媒体事实与记录一致
- **THEN** CLI 返回 `native-export` 等级的机器可读通过结果，并保留真实人工审查状态

#### Scenario: Acceptance binding or artifact drift is detected
- **WHEN** 验收记录缺少草稿摘要或批准 ID，或者输出/联系表摘要及媒体属性发生漂移
- **THEN** CLI 失败关闭，不得把文档中的“导出成功”提升为可交付证据

### Requirement: Every editor control has a semantic execution route
系统 SHALL 为目标剪映版本中观察到的每个工具栏按钮分配稳定语义 ID、状态探测方式、参数合同、首选执行路由、完成证据和回退路由。首选顺序 MUST 为草稿协议直接编辑、原生 Runtime 控制、可访问性 GUI Adapter、视觉兜底。

#### Scenario: Direct draft operation exists
- **WHEN** 按钮语义可由分割、裁剪、移动、删除、轨道、关键帧、滤镜、特效或转场等已有草稿操作完整表达
- **THEN** CLI 直接修改隔离草稿并输出前后差分，不启动编辑器或模拟点击

#### Scenario: Track mute is applied through the draft protocol
- **WHEN** 调用方以精确轨道 ID 设置轨道静音状态
- **THEN** CLI 只设置草稿轨道 `attribute` 的静音位，保留其他位、未知字段和每个片段原有音量；返回旧/新静音状态及原始位值，并通过重新读取草稿确认结果
- **AND** 无效轨道 ID 或非法 `attribute` 必须在事务提交前失败，双镜像保持不变；在真实剪映及发布二进制验收前，该控件不得标记为 `supported`

#### Scenario: Control only changes editor session state
- **WHEN** 按钮控制撤销栈、磁吸、联动、录音或时间线缩放等编辑器会话状态
- **THEN** CLI 通过版本绑定的 Runtime Control 探测并设置该状态，不把会话状态伪造为草稿字段

#### Scenario: Button identity is unknown after an editor update
- **WHEN** 当前编辑器版本无法将按钮映射到已验证的语义 ID
- **THEN** 系统记录未知控件并停止自动执行，不复用旧坐标或按图标外观猜测

#### Scenario: The same semantic control appears more than once
- **WHEN** 锁定、显示、页签切换或波形选项等语义在不同轨道或不同入口重复出现
- **THEN** 系统在界面表面清单中逐实例登记，并全部引用受校验的语义控制；不得因为语义相同而漏掉可操作入口

#### Scenario: A control expands into child actions
- **WHEN** 页签菜单、工具下拉、联动下拉、预览轴下拉、更多设置或右键菜单可以展开
- **THEN** 父入口和每个子操作分别进入清单；父入口已登记不能作为子操作完整覆盖的证明

#### Scenario: GUI fallback reports success
- **WHEN** 可访问性或视觉兜底完成一个界面动作
- **THEN** CLI 仍须用草稿、会话状态或产物证据验证效果，单次点击成功不能满足完成门禁

### Requirement: Homepage local-folder lifecycle is atomic and lossless
系统 SHALL 通过 Rust CLI 管理首页本地文件夹的查询、创建、移入“最近删除”、回收站查询与恢复。写操作 MUST 以精确 ID 为目标，在剪映运行时拒绝修改真实配置，保留未知字段，在任何文件变更前生成可恢复快照，并对相关配置文件执行原子提交或全量回滚。

#### Scenario: Empty leaf folder is recycled and restored
- **WHEN** 用户按精确文件夹 ID 将一个无子文件夹、无草稿映射的本地文件夹移入“最近删除”，然后按精确回收 ID 恢复
- **THEN** CLI 返回操作前后的文件夹与回收条目计数、绑定 ID 和快照路径，最终活动列表恢复该文件夹且回收条目消失

#### Scenario: Unknown fields survive the lifecycle
- **WHEN** 文件夹、映射或回收配置包含当前 CLI 尚未建模的根字段或条目字段
- **THEN** 列表、回收与恢复全程保留这些字段及其值，不通过强类型重序列化丢失未知数据

#### Scenario: Folder mutation cannot be proven safe
- **WHEN** 目标 ID 缺失或冲突、目标包含未建模的草稿/子文件夹回收语义、配置结构不完整，或真实剪映配置正被运行中的编辑器使用
- **THEN** CLI 以结构化错误失败关闭，不写入任何相关文件，不按名称猜测目标，也不把 GUI 中条目消失视为恢复成功

#### Scenario: Multi-file commit fails
- **WHEN** 原子提交中任一配置文件无法落盘、重命名或回读校验
- **THEN** CLI 从操作前快照恢复所有已触及文件，返回失败阶段与快照身份，不留下部分更新状态
