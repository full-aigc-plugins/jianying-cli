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

### Requirement: Every editor control has a semantic execution route
系统 SHALL 为目标剪映版本中观察到的每个工具栏按钮分配稳定语义 ID、状态探测方式、参数合同、首选执行路由、完成证据和回退路由。首选顺序 MUST 为草稿协议直接编辑、原生 Runtime 控制、可访问性 GUI Adapter、视觉兜底。

#### Scenario: Direct draft operation exists
- **WHEN** 按钮语义可由分割、裁剪、移动、删除、轨道、关键帧、滤镜、特效或转场等已有草稿操作完整表达
- **THEN** CLI 直接修改隔离草稿并输出前后差分，不启动编辑器或模拟点击

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
