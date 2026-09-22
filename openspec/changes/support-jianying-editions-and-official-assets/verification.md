# 验证记录

## 2026-09-22：插件 GUI Adapter、只读版型观察与 Harness 路由

实现位于独立插件仓库 `jianying-edit-plugin` 的 OpenSpec 变更
`orchestrate-jianying-native-capabilities`，CLI 继续只拥有 Rust 权益裁决、收据登记和预检，
没有把 GUI 坐标、账号 Cookie 或 Token 引入 CLI。

- 任务 3.1：插件已定义严格的 NativeCapabilityCatalog、ResourceCandidate、
  ResourceReceipt、NativeActionEvidence、查询/筛选/取得/预览/应用/检查合同；WorkflowPlan
  只携带语义资源 ID、依赖与 capability，不携带屏幕坐标。
- 任务 3.2：新增摘要绑定的 `jianying-native-edition-observation/v1`，分别记录应用显示名、
  登录状态、账号版型、权益状态、能力集合与有效期。应用名为“剪映专业版”而账号证据未知时，
  结果仍为 `unknown`。单项资源下载状态继续由 ResourceCandidate 独立记录。
- 任务 3.4：Harness `native-resource-route` 使用现场观察收紧查询。基础账号把 `any`
  收紧为 `basic`；专业请求、未知/过期权益、未登录或缺失 capability 都返回结构化暂停和
  可操作恢复建议，不静默替换素材。取得后仍由发布 Rust CLI `media official register/preflight`
  对权益、字节身份和用途做第二次裁决。
- 安全边界：候选、目录证据、Adapter 身份、取得证据和持久化结果递归拒绝 Token、Cookie、
  Password、Credential、Authorization 与 API Key 字段。

验证证据：

- 插件 Node 全量：312 项中 305 通过、7 条真实制品/宿主条件测试跳过、0 失败。
- Python：70/70。
- 从锁文件 URL 重新下载不可变 `jianying-cli v1.6.19` macOS arm64 bundle，归档 SHA-256
  `efe1e939485cf109659448ae23413c966a9ec29995c099adf20485e9c550e2bd` 与锁一致。
- 真 bundle 黑盒 4/4：无 Cargo 安装握手、未下载官方资源经 Fake GUI 进入发布 Rust CLI、
  基础/专业/限免过期/用途冲突矩阵、语义控制目录编译。
- 插件分发校验、三个相关 OpenSpec strict 与差异检查通过。

保留任务：

- 任务 3.3：Adapter 现在支持两种互斥物化身份。视频/音频等使用常规文件字节长度与
  SHA-256；贴纸、特效、转场、文字模板等使用 `draft_resource`，要求隔离草稿中的节点 ID
  唯一，并绑定规范化节点 SHA-256 与原始 `draft_content.json` SHA-256。注册与预检分别
  选择文件参数或 `--draft`，不伪造本地文件。候选、观察证据和持久化结果继续执行秘密字段拒绝。
- 任务 3.5：同一个真实 `v1.6.19` bundle 测试矩阵已覆盖基础、专业、限免过期、用途冲突、
  未知版型、仅预览、下载失败和真实过期权益。前置拒绝不会触发 CLI；过期权益由真实发布
  Rust CLI 返回 `entitlement_expired`。额外黑盒用 Fake GUI 将内嵌画面特效交给发布 CLI，
  `draft_resource` 登记和预检均通过。
- 真实剪映与正式三宿主门禁仍由第 4 节管理。

## 2026-09-22：任务 2.10 剪映专属组合动作状态机

组合工作流实现位于独立插件仓库 `jianying-edit-plugin`，CLI 仍保持原子能力与权益裁决边界。
插件的 `smart_package_orchestration.mjs` 现在显式区分 `smart_package`、`smart_b_roll`、
`smart_rough_cut`（智能粗剪）和 `script_rough_cut`（文案粗剪），并为每个请求固化
`plan -> preview -> approve -> apply -> verify` 生命周期合同。

- `plan`：请求冻结动作类型、风格、输入、允许轨道、受保护内容、预算、预览要求和最长时长。
- `preview`：提案绑定请求摘要、预览摘要、可解释变更集和估算费用。
- `approve`：只接受绑定精确请求摘要、变更集摘要且未过期的人工批准。
- `apply`：应用前创建快照；不明确或失败的应用立即恢复并输出 `recover=restored` 状态轨迹。
- `verify`：重新读取实际变更；对白、品牌、锁定镜头、范围或摘要漂移均拒绝并恢复快照。

兼容性处理：WorkflowPlan 继续使用已冻结的 `preview -> human review -> apply -> independent verify`
四步子图；其中请求构建就是 `plan`，human review 的强类型输出就是 `approve`，因此没有为了命名
重写已发布 DAG。旧 v1 请求仍可读取，新建请求总是输出动作与生命周期字段。

验证证据：

- 失败优先测试先因缺少 `mapSmartPackageOperation` 导出而失败，随后最小实现转绿。
- 智能粗剪、文案粗剪均通过动作映射、严格 Schema 与五阶段顺序验证；未知动作 fail closed。
- 成功执行输出五阶段 `state_trace`，且轨迹进入 evidence SHA-256；不明确应用输出
  `plan/preview/approve/apply/recover`，独立验证失败输出
  `plan/preview/approve/apply/verify/recover`，两类恢复都要求 Adapter 回读 `restored`。
- 定向测试 6/6 通过；注入锁定的不可变 `v1.6.19` release bundle 后，插件 Node 全量
  315 项中 311 通过、4 条真实宿主条件测试跳过、0 失败；Python 70/70，分发校验通过。

该任务只证明状态机与恢复合同，不提升 `runtime.smart_package` 的真实剪映 Canary 等级；真实 Vlog
预览/批准/应用/冷重开/质量评审仍由插件任务 4.5 和本变更第 4 节验收。

## 2026-09-22：任务 2.12 的可机读展开门禁（尚未完成）

本轮没有操作用户界面，也没有依据图标猜测子菜单。CLI source candidate 将 7 个可展开父入口从
隐式命名约定升级为结构化 `expansion` 合同：页签菜单、选择工具、联动、预览轴、更多设置、
片段右键和轨道右键分别记录类型、状态、已知子项和证据。coverage 当前如实报告
`expandable_parents=7`、`enumerated_parents=0`。

- “更多时间线设置”保存截图可见的 4 个子项与父子关系，但因未取得可访问性名称保持 `partial`。
- 其余 6 个父入口保持 `pending_accessibility` 且子项为空。
- Rust 校验器拒绝父入口计数漂移、未知子项、错误父子回指，以及缺少可访问性名称却声称
  `enumerated` 的伪完成清单。
- 插件 Schema 向后兼容旧发布清单，并在新合同存在时以显式 `expansion.status` 为准；
  `partial` 即使已有截图子项也仍输出 `unexpanded_parent` gap。
- RED/GREEN 证据：CLI 黑盒最初因缺少 expansion coverage 返回 `null`，插件最初因未知
  `expansion` 字段拒绝全部 7 项控制测试；实现后 CLI 黑盒 1/1、Rust 负向单测 2/2、
  插件控制编排 7/7 通过。
- 完整回归：为避免关闭或干扰用户正在运行的真实剪映，仅在测试子进程 PATH 前置一个返回空
  进程列表的临时 `ps` 替身；生产代码和真实进程不变。该隔离下 `cargo test --workspace` 全绿，
  `cargo fmt --check` 通过。插件注入不可变 `v1.6.19` bundle 后 Node 311 通过、4 个真实宿主条件
  测试跳过，Python 70/70、分发校验与双方 OpenSpec strict 通过。

任务 `2.12` 保持未勾选：仍须在精确版本剪映中实际展开 7 个父入口，逐个取得子操作与
可访问性名称，再将 `enumerated_parents` 提升到 7。父入口、视觉标签或测试 fixture 不能替代该证据。

## 2026-09-22：真实剪映验收证据二进制复核门禁

新增 `jianying --json runtime acceptance verify <record>`。该命令不控制 GUI，只从二进制重放
真实验收证据：校验精确编辑器运行时身份、正式 CLI 版本/Release/摘要、草稿摘要、批准 ID、
来源许可证清单、冷重开/连续播放/原生导出观察，并重新计算原生输出、联系表与来源清单 SHA-256；
同时用 `ffprobe` 复核视频/音频 codec、画幅、帧率、时长、声道和采样率。

- RED：新增黑盒测试最初均因 `runtime acceptance` 不存在失败。
- GREEN：完整 fixture 返回 `highest_evidence_level=native-export`；移除 `approval_id` 和篡改原生输出
  均失败关闭。定向黑盒 2/2、Runtime CLI 回归 10/10 通过。
- `--require-human-review` 可用于最终交付门禁；默认模式仍保留 `human_review=pending`，不把技术导出
  自动升级为人工内容验收。
- 真实园区招商记录与七个既有导出文件的路径、大小和 SHA-256 已重新核验；园区样片记录当前被新
  二进制正确拒绝，首个缺口是 `environment.editor.runtime_identity`，之后仍需草稿摘要和可追溯批准
  绑定。旧 Vlog/课程/婚礼制品虽摘要一致，但存在叙事不足或占位画面，不得据此提升内容门禁。

因此任务 4.3 继续保持未勾选：实现和自动回归已经具备，但现有真实记录尚未满足自身
`provenance/EVIDENCE_MODEL.json` 对 native-export 的完整绑定要求。不得通过补写不可追溯的批准 ID
回填完成状态。
