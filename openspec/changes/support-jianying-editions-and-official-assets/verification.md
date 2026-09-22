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

## 2026-09-22：精确版本 GUI 可访问性与片段右键实测（部分进展）

在剪映专业版 `11.5.13243`（build `11.6.0-beta2`）的真实 13 秒验收草稿中，已通过
macOS 可访问性树取得左侧工具栏名称：`editor.undo`、`editor.redo`、`cutoff`、
`cutLeft`、`cutRight`、`del`、`aiBeat`、`mark`、`freeze`、`reverse`、`mirror`、
`rotate`、`crop`、`trimClip`、`smartExtend`、`smartRoughCut`、
`cutClipBySceneEditDetection`、`scriptRoughCut`，以及 `audioRecord`、
`TimeLineRulerAdsorb`。

片段右键父入口已真实展开，并枚举到复制、剪切、复制属性、粘贴属性、删除、选中同色片段、
AI 生成、AI 编辑、基础编辑、智能镜头分割、智能粗剪、智能剪口播、识别字幕/歌词、
智能生成 AI 音效、分离音频、视音频对齐、新建复合片段（子草稿）、新建多机位片段、
保存为我的预设、创建组合、解除素材包、导出所选片段、停用片段、重选时段、替换片段、LUT、
链接媒体、打开文件所在位置、编辑特效、显示关键帧变速曲线、时间区域和渲染。

同时实操验证了播放/停止、轨道锁定/显隐、音轨静音/独奏、`cutoff` 后撤销、真实关闭后冷重开、
从 `00:00:00:00` 连续播放至 `00:00:13:00`，以及“文件 → 导出”的本地原生导出。新 MOV 经
`ffprobe` 复核为 H.264 + AAC、1080x1920、30fps、13.000 秒，SHA-256 为
`1febdad07d2e82a44fe74b0ce7c6825b4b3d1d3b9812d859278e083c2c9c0608`。

任务 `2.12` 仍保持未勾选：页签菜单、选择工具、联动、预览轴、更多设置全部子项和轨道右键
尚未递归枚举，片段右键子项也尚未写入正式 machine inventory 并通过父子引用回归。不得用本段
人工记录提前把 `enumerated_parents` 提升为 7。

## 2026-09-22：主页、权益与宿主版本实测（新增边界）

真实主页补验确认团队版权益、个人/小组云空间、即梦资产、草稿搜索/视图/排序/文件夹/回收站、
AI 创作工作台、全局设置、网络检测、版本与第三方版权均是独立原子操作面。团队版页面显示容量、
席位和权益对比；小组云空间另有管理、上传草稿/素材和传输入口，不能因为应用名称为“剪映专业版”
就把所有资源能力推断为已授权。

版本窗口确认当前进程为 `11.6.0-beta2`；检查更新后宿主自动下载 `11.6.0 内测版 3`，但未执行
重启安装。该结果要求 Runtime Profile 与 GUI Canary 绑定精确运行版本并显式报告待安装更新。
现有 CLI 尚无完整 `home.*`、主页草稿管理、设置读取和工作台门禁 machine inventory，因此本次
只能作为后续命令规格证据，不能提升任务 `2.12` 或真实资源应用状态。

## 2026-09-22：首页本地文件夹 Rust 协议命令

新增 `home folder list/create/recycle/list-recycled/restore`，并将该命令组限定为
macOS/Windows。实现以精确文件夹 ID 或回收 ID 定位，直接编辑已观测的
`LocalDraftFolder` v1.0 JSON 协议，不依赖坐标、图标或视觉识别。

- RED：5 条 CLI 黑盒用例初始全部因 `home` 命令不存在而失败。
- GREEN：5/5 黑盒通过，覆盖列表、创建、空叶子文件夹移入最近删除、回收站回读、
  恢复、精确 ID、确认门禁、host-read-only、同名冲突、未知根/条目字段保留与
  非空文件夹失败关闭。
- 原子性：每次写入在配置目录外创建快照和工作副本，全量回读校验后再以目录
  rename 提交；注入激活失败的单元测试证明原始四个配置文件逐字节恢复。
- 安全性：真实平台配置在剪映/CapCut 运行时拒绝写入，自定义隔离 fixture 不受用户
  正在运行的剪映干扰；符号链接、特殊文件和未知版本均失败关闭。
- 真实读回：本地 source candidate 在不写入的情况下读取真实配置，返回活动文件夹
  `bb68c5dc-f614-4dfb-b388-dbe8e2099d39` / `新建文件夹` 一项且回收站为空，与上一轮修复后
  的配置一致。
- 定向 `cargo clippy --all-targets --all-features -- -D warnings`、OpenSpec strict 与所有上述测试通过。

`home.folder_lifecycle` 保持 `partial`：任务 4.6 仍需在不可变发布二进制上完成真实空
文件夹的回收、恢复、冷重开与前后状态复核。非空文件夹的原生 wire 格式仍未获得差分证据，
因此本轮没有猜测 `draft_mapping_recycle_bin.json` 结构，也没有提升为 `supported`。

## 2026-09-22：不可变发布二进制首页文件夹闭环

不可变 Release `v1.6.23` 已在 GitHub 发布并通过 release attestation 验证；tag 与发布前、发布后
远端复核均解析到 commit `ff1cfbe6edce489858ba8e8d60c79728ba1fc843`。darwin-arm64 压缩包
SHA-256 为 `35e3969046c6f99a41e717517775a18feead8939d30420c17d904e2fe4393392`，
其中发布二进制 SHA-256 为 `06d9be0ee0344d130ec33a8cfb5922b1399f729e91861e8e7563271f8271a940`。

该发布二进制在真实剪映 `11.6.0-beta3` 与真实 `LocalDraftFolder` v1.0 上完成：

1. 运行中只读列出精确文件夹 ID，并确认所有配置文件摘要前后相同；
2. 运行中请求回收被结构化 `editor_running` 拒绝，所有配置文件逐字节相同；
3. 正常停止编辑器后，将空叶子文件夹按精确 ID 移入最近删除，返回安全快照；
4. 独立执行活动列表与回收站回读，确认 `1 → 0` 与 `0 → 1`；
5. 后台冷重开真实剪映，再次回读仍为活动 0、回收 1；
6. 正常停止后按精确 recycle ID 恢复，CLI 仅在活动列表重新出现且回收站清空后返回成功；
7. 第二次后台冷重开后，最终状态稳定为活动 1、回收 0，文件夹 ID、名称与创建时间保持不变。

此前 GUI 恢复操作出现过“回收站条目消失但活动列表未返回”的歧义，该观察未被计为成功。本次
CLI 的提交后全量回读与插件的独立二次回读均要求目标 ID 位于正确集合，因此同类中间态不会被误报。
完整结构化证据固化在 `provenance/HOME_FOLDER_LIFECYCLE_ACCEPTANCE.json`。

据此任务 4.6 完成，`home.folder_lifecycle` 在已声明范围内提升为 `supported`：支持列表、创建、
空叶子文件夹回收、回收站列表和恢复；非空、含子文件夹或关联草稿的文件夹仍以
`nonempty_folder_schema_unverified` 失败关闭，未擅自猜测尚未观测的原生 wire 格式。

## 2026-09-23：轨道静音草稿协议原子命令（任务 2.15）

上游 Apache-2.0 固定参考把轨道静音编码为 `attribute` 的最低位；本项目独立实现
`jianying timeline track-mute <draft> <track_id> <true|false> --json`。命令只按精确轨道 ID
变更该位，保留其他位、未知轨道字段和片段原始音量，经隔离工作副本验证、双镜像原子提交后，
重新读取草稿确认属性值。无效轨道 ID 或非法属性在提交前失败，原草稿字节不变。

- RED：新增黑盒用例初次因 `track-mute` 命令不存在而失败。
- GREEN：`timeline_cli_contract` 17/17、`agent_contract` 17/17、
  `runtime_cli_contract` 定向目录断言均通过；`cargo clippy --all-targets --all-features -- -D warnings`、
  `cargo fmt --check`、OpenSpec strict 与 `git diff --check` 通过。`cargo build --release --locked`
  构建 `v1.6.25`，发布模式二进制通过 `verify_release_runtime.py --binary`；Python 32/32 通过。
- 全量 `cargo test --workspace` 首次被宿主正在运行的 `VideoFusion-macOS` 触发草稿拒写门禁；
  未关闭或操作编辑器。仅在第二次测试进程的 `PATH` 前置合成 `ps` 替身后，工作区全量测试通过。
  此隔离不改变生产二进制的进程探测和运行中拒写逻辑。
- `track.audio.mute` 与 `timeline.track_mute` 仍标为 `partial`：尚无新命令的不可变发布
  二进制和真实剪映冷重开/播放差分证据，不能将本地草稿结构测试冒充界面验收。

## 2026-09-23：不可变静音二进制复核与轨道重命名（任务 2.16）

`v1.6.25` 的不可变 CLI Release 已发布，tag 在发布前后均解析到
`eafb33587341800fd348e780240edab53a1033c3`。darwin-arm64 发布二进制
SHA-256 为 `9b7f30b7576756190d307eb0e7bd1e34c63826eb6a41bb7ff46c9f2780a47439`。
该真实发布二进制在隔离三秒合成音频草稿上执行 `track-mute false → true → false`，
轨道 `attribute` 为 `0 → 1 → 0`，片段音量保持 `1.0`，两份草稿镜像逐字节一致，
`project verify` 无问题。剪映处于运行中，因此本次合成草稿黑盒测试仅对测试进程
隔离 `ps` 探测；生产二进制未修改，真实剪映冷重开、播放和导出仍未验收。

新增 `jianying timeline track-rename <draft> <track_id> <name> --json`：只按精确轨道 ID
写入名称及 `is_default_name=false`。未知轨道字段、`attribute` 位和片段音量保持不变；
无效 ID、空白名称或非法旧标记在事务提交前失败，两份镜像字节不变。测试先因命令不存在
RED，再 GREEN；`timeline_cli_contract` 18/18、`runtime_cli_contract` 10/10、
`cargo test --workspace --locked` 全量通过（合成草稿测试进程隔离宿主编辑器探测），
Python 32/32、Clippy、格式校验及 OpenSpec strict 通过。

本地 `v1.6.26` release-mode 二进制经 `verify_release_runtime.py` 验证，在隔离三秒合成
音频草稿上完成重命名并双镜像回读：名称均为“配乐轨道”、默认命名标记均为 `false`、
属性位仍为 `0`，`project verify` 无问题。此阶段尚非不可变 `v1.6.26` 发布制品，
更未在真实剪映冷重开后验证界面呈现；`track.rename` 与
`timeline.track_rename` 必须保持 `partial`。

## 2026-09-23：不可变 v1.6.26 正式制品差分补验

`v1.6.26` Release 已发布且仓库 Immutable Releases 开启；tag 在发布前后均解析到
`0a1affe71d27385f07663f5fe1c8a351d0cca3c8`。源码差分、三平台工作区测试、
发布二进制、SBOM 和 10 个 Release 资产通过正式流水线。Release index SHA-256 为
`0258288154dff1127a8583b37d6149f717c3f77ac53c91c903ef0199e1a17b53`；
darwin-arm64 压缩包 SHA-256 为
`7d2e9a61391b72adabd0aa9ce906f134975181751081b577a7a37a3e7fea5893`，
其中二进制 SHA-256 为
`6e9047260fdb9b86602c006c08fa70f8c744968b41920d1b8c421d55b289a0fd`。
`gh release verify` 与该压缩包的 `gh release verify-asset` 均通过。

下载该正式二进制后，在隔离三秒合成音频草稿执行 `project quickstart →
timeline track-rename → project verify`。两份草稿镜像逐字节一致，名称均为
“正式制品配乐轨”，`is_default_name=false`，`attribute=0`、片段音量 `1.0`，
`project verify` 无问题。由于真实剪映正在运行，合成草稿命令在测试进程中隔离
编辑器进程探测；这不等于真实剪映冷重开或界面回读。发布二进制能力清单继续返回
`timeline.track_rename=partial`，不作为自动 GUI 路由依据。

## 2026-09-23：首页文件夹重命名（任务 2.17）

在已观测的 `LocalDraftFolder` v1.0 协议上新增 `jianying home folder rename`。
测试先因子命令不存在失败，再实现精确 ID、同级重名拒绝、非空且无控制字符名称、
父子关系和草稿映射不变、未知字段保留、四文件事务快照、提交后按 ID 回读。
提交器现在还将四个实际回读文件与拟写文档比较，不一致时执行既有全量回滚。
合成配置测试覆盖带草稿映射的文件夹、无效目标、重复名称和宿主只读失败，
不写入真实剪映配置，也不操作 GUI。新命令能力与机器命令目录均标为 `partial`：
正式发布二进制和真实剪映冷重开验证仍是独立门禁。

本地 `v1.6.27` release-mode 二进制通过 `verify_release_runtime.py`，随后对隔离的
`LocalDraftFolder` v1.0 合成配置执行重命名。返回 `status=renamed`、活动文件夹和
草稿映射计数均保持 `1`，CLI 独立 `home folder list` 回读到新名称、原 ID/父节点、
新 `modifiedTime` 和未知字段；命令目录仅将新增重命名标为 `partial`，已有恢复命令
仍为 `supported`。这不是不可变 Release 制品，也不是剪映 GUI 冷重开证据。
