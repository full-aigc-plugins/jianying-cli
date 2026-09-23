## 1. 来源、许可证与基线门禁

- [x] 1.1 固定 pyJianYingDraft `c3318066`、capcut-cli `49f70e3b` 和自主 headless 能力目标，生成含仓库、提交、许可证、用途与哈希的 `SOURCE_MANIFEST`
- [x] 1.2 建立逐文件可迁移范围清单，明确 Apache-2.0、MIT 的声明义务，并把非商业 headless 源码、测试、蓝图、资源和实现常量列入禁止输入
- [x] 1.3 为来源不明 fixture、受限来源片段和缺失版权声明增加 CI 失败门禁，并用正反样例验证
- [x] 1.4 冻结当前 `jianying-cli-plan/v1`、现有命令和 55 个差分场景的 golden 基线
- [x] 1.5 建立功能级、命令级、数据结构级 parity 矩阵，为每项记录来源版本、Rust 落点、测试、证据等级和状态
- [x] 1.6 将 capcut-cli 的 86 个基线命令逐项录入能力矩阵，并添加“无空项、无重复项”的机器校验

## 2. Cargo workspace 与统一领域模型

- [x] 2.1 建立 domain、schema、draft、store、media、runtime、jobs、mcp、cli crates，并用兼容 facade 保持现有入口可编译
- [x] 2.2 实现 Project、Timeline、Track、TimeRange、FrameRate 与片段标识等领域值对象及不变量测试
- [x] 2.3 以 serde tagged enum 实现视频、音频、文字、贴纸、滤镜、特效和复合片段，拒绝类型不兼容字段
- [x] 2.4 实现素材、资源引用和 EditOperation 强类型模型及序列化 round-trip 测试
- [x] 2.5 实现整数微秒和帧网格量化，输出 `QuantizationReport` 并覆盖拒绝不安全量化的测试
- [x] 2.6 实现已有草稿 typed view + lossless raw envelope，并验证未触及未知字段、素材节点和镜像关系不变

## 3. Job/Plan Schema 与兼容转换

- [x] 3.1 定义 `jianying-job/v2` JSON Schema，覆盖 create、edit、inspect、verify、publish、export 和 batch
- [x] 3.2 实现 `jianying-cli-plan/v1` 到 v2 领域模型的纯转换器及字段级诊断
- [x] 3.3 用冻结 fixture 验证 v1 输入经转换后生成与基线语义等价的草稿
- [x] 3.4 发布 schema、示例、版本兼容规则和机器可读 capability manifest
- [x] 3.5 增加未知 schema、破坏性版本和不兼容 capability 的结构化失败测试
- [x] 3.6 将 `job run edit` 接入非空强类型操作列表，完成文字替换、片段移动/删除、隔离副本原子提交及源草稿逐文件不变黑盒验证

## 4. pyJianYingDraft 草稿协议无损迁移

- [x] 4.1 迁移草稿元数据、画布、时间线、轨道和片段的 wire model，并建立规范化差分测试
- [x] 4.2 迁移视频、音频、文字、贴纸、滤镜、特效、转场、蒙版、动画和关键帧资源语义
- [x] 4.3 迁移素材注册、资源目录、模板和草稿库镜像关系，验证 ID 与引用完整性
- [x] 4.4 迁移字幕、样式、速度、音量、裁剪、变换和组合编辑语义
- [x] 4.5 为每类协议对象增加 Python 参考输出与 Rust 输出的完全相等、规范化等价或批准差异记录
- [x] 4.6 扩充边界 fixture，覆盖空轨、重叠、负偏移、变速、缺失资源、Unicode 和未知字段写回
- [x] 4.7 在完整协议矩阵达到门禁前阻止“pyJianYingDraft parity 完成”发布声明
- [x] 4.8 将 clip、crop、transform、关键帧、蒙版、色度、背景、混合、动画、转场、音频效果与文字样式提升为独立领域值对象，并完成 v1 双向投影
  - 第一批增量（2026-09-22）：`ClipSettings`、`CropSettings`、`Transform` 进入 `jianying-domain`，v1 转换和 Domain→Plan 投影保持平面 Job v2 JSON 兼容；55/55 固定提交差分仍逐字节匹配。当时其余值对象尚未迁移，因此未勾选任务。
  - 第二批完成（2026-09-22）：关键帧、蒙版、色度抠图、背景填充、混合模式、动画、转场、淡入淡出/音频效果和文字样式均成为可独立校验的领域值对象；视频、音频、文字、贴纸片段完成 v1→Domain→Plan 双向投影，Job v2 schema 同步声明全部字段。55 个冻结 fixture 的 4.8 字段逐片段等价，固定 `c3318066` Python/Rust 草稿差分 55/55 通过且报告与基线逐字节一致。
- [x] 4.9 让 `jianying-job/v2` create 仅通过 `DraftProject -> Domain-to-Wire` 编译；v1 compatibility 只作为输入转换证据，不再承担生产执行
  - 完成（2026-09-22）：普通 `jianying-cli-plan/v1` 已只通过转换后的 `DraftProject` 执行；黑盒冲突载荷测试证明 compatibility 中的名称和文字不能覆盖领域项目。迁移同时补齐轨道稳定 ID 与显示名的分离语义。
  - `capcut-cli-compile/v1` compatibility-only Job 先解析为严格 `CompileSpec`，再将基础轨道转换成 `DraftProject` 后进入同一 Domain-to-Wire 编译；九类后处理只消费强类型 `CompileOperation`。单稿、批量和 Job v2 黑盒均确认 `compiler=draft_project_to_wire`。
- [x] 4.10 扩展 `EditOperation` 与共享 handler，完成新增素材/片段、轨道操作和全部强类型语义的隔离事务执行
  - 完成（2026-09-22）：Job v2 新增稳定 ID 的 `add_track`、`remove_track`、`reorder_track`，并将本地视频/音频/图片 `add_material` 与完整强类型 `add_segment` 接入隔离 edit handler。片段只通过 `DraftProject -> Domain-to-Wire` 编译，导入时复制主素材与伴随引用闭包；关键帧、蒙版、色度、背景、混合、动画、转场、淡入淡出等字段均由黑盒产物验证。未声明、重复、缺失或未消费素材结构化失败，源草稿、最终输出和临时草稿保持原子边界。
- [x] 4.11 实现素材替换长短策略、文本样式区间重算、轨道导入 ID 重映射与引用闭包，并建立 Python/Rust 差分
  - 完成（2026-09-22）：固定 pyJianYingDraft `c3318066` 的 8 类长短策略与文字样式观察值进入 `PYJYD_TEMPLATE_DIFFERENTIALS.json`；Rust 按相同顺序执行裁头、裁尾、裁尾对齐、居中收缩、向前/向后延长、推动后续和截断素材尾部。文字替换使用 UTF-16 码元比例重算并记录相对 Python code-point 的获准增强差异。
  - 按片段替换会拆分共享素材身份并通过 `MutationPlan` 原子提交；失败黑盒证明草稿字节和素材文件数均不变。轨道导入递归收集普通字段和 JSON 字符串中的素材引用，重映射轨道/片段/素材 ID、复制本地文件且保留无关 JSON 原文；删除源草稿后目标仍通过 bundle 校验。
- [ ] 4.12 将 pyJianYingDraft 四个 `partial` 提升为有差分和真实剪映证据的 `supported`；CI 必须拒绝完成声明与 parity 状态不一致

## 5. 领域命令树与机器契约

- [x] 5.1 实现 project、timeline、media、captions、template、store、render、job、runtime、approvals、config 和 mcp 命令组骨架
- [x] 5.2 实现全局 `--json`、`--profile`、`--no-color`、`--log-level` 和稳定退出码
- [x] 5.3 实现成功输出与统一错误信封，验证 JSON 模式 stdout 只有一个文档且诊断只写 stderr
- [x] 5.4 实现机器可读命令/capability 目录，包含参数 schema、读写等级、平台条件和输出 schema
- [x] 5.5 将现有顶层命令改接共享 handler 并提供带弃用提示、无语义漂移的兼容别名
- [x] 5.6 实现 doctor、status、audit 和 shell completion，并覆盖凭据与敏感路径脱敏测试

## 6. capcut-cli 全能力覆盖

- [x] 6.1 完成项目读取、信息、版本、差异、描述、迁移、拼接和快速创建能力及黑盒差分测试
- [x] 6.2 完成轨道/片段查询、新增、设置、裁剪、移动、拆分、变速、音量和组合编辑能力
- [x] 6.3 完成媒体探测、素材新增/替换/重链、场景、静音、重拍、TTS 和音效能力
- [x] 6.4 完成字幕列表、设置、导入、导出、样式、单条字幕和翻译能力
- [x] 6.5 完成模板列表、保存、应用、预设、复制和轨道导入能力
- [x] 6.6 完成草稿库列表、登记、重命名、同步、备份、恢复、解密和目录能力
- [x] 6.7 完成代理渲染、批量渲染、批处理输入和服务模式能力
- [x] 6.8 为 86 项映射逐项添加测试与证据链接，CI 拒绝 `partial` 或 `external` 状态被宣称为完整支持
- [x] 6.9 补齐 capcut-cli `materials` / `material` 只读能力，建立固定提交黑盒差分，并同步 parity 与逐命令证据
- [x] 6.10 补齐 capcut-cli `export-timeline` OpenTimelineIO 导出，建立固定提交黑盒差分并同步逐命令证据
- [x] 6.12 补齐 capcut-cli `import-timeline` 的新建/追加、占位素材、字幕 Marker、dry-run 与不支持特性报告，并建立固定提交差分证据
- [x] 6.13 补齐 capcut-cli `timeline` 结构化布局视图及列坐标计算，纳入固定提交黑盒差分
- [x] 6.14 补齐 capcut-cli `duplicate` 的素材/伴随资源/关键帧深拷贝和既有轨道放置，以及 `remove` 的孤儿资源清扫、保留选项与 dry-run，并建立固定提交差分
- [x] 6.15 补齐 capcut-cli `prune` 的 surviving-reference 保守清扫、逐素材类型统计与 dry-run，并建立固定提交差分
- [x] 6.16 补齐 capcut-cli `matting` 的视频素材级智能抠像开关、缓存字段保留与共享片段报告，并建立固定提交差分
- [x] 6.17 补齐 capcut-cli `chroma` 的视频片段色度键素材创建、参数钳制、引用挂载与定向移除，并建立固定提交差分
- [x] 6.18 补齐 capcut-cli `mask` 的 CapCut/JianYing 资源解析、三种素材字段选择、几何参数、单 mask 约束与跨字段关闭，并建立固定提交差分
- [x] 6.19 补齐 capcut-cli `bg-blur` 的四级画布模糊映射、既有 canvas 引用替换与关闭语义，并建立固定提交差分
- [x] 6.20 补齐 capcut-cli `audio-fade` 的秒到微秒转换、音频轨约束、ID 前缀解析与重复应用替换引用语义，并建立固定提交差分
- [x] 6.21 补齐 capcut-cli `add-cover` 的图片存在性门禁、毫秒时间点、顶层 cover wire 替换与默认时间语义，并建立固定提交差分
- [x] 6.22 补齐 capcut-cli `add-filter` 的固定目录、原始资源 ID、强度、全时间线、独立 filter 轨道与素材 wire 语义，并建立固定提交差分
- [x] 6.23 补齐 capcut-cli `bubble-text` 的文字片段前缀解析、固定目录/原始 ID、`filters` 素材、文字素材双写与重复应用替换引用语义，并建立固定提交差分
- [x] 6.24 补齐 capcut-cli `add-effect` 的固定/剪映目录、原始 ID、参数、强度、全时间线、片段绑定与 effect 轨道语义，并建立固定提交差分
- [x] 6.25 补齐 capcut-cli `crop` 的只读查询、显式矩形、比例计算、重置、dry-run、片段前缀与视频/图片素材写入语义，并建立固定提交差分
- [x] 6.26 补齐 capcut-cli `cut` 的时间窗提取、片段裁边与重基准、源时间范围调整、空轨与孤立素材清理，并建立固定提交差分
- [x] 6.27 补齐 capcut-cli `keyframe` 的属性别名、值归一化、单点/JSONL 批量、线性/贝塞尔/hold 缓动与邻接句柄更新语义，并建立固定提交差分
- [x] 6.28 补齐 capcut-cli `transition` 的 CapCut/剪映双目录、默认/显式时长、片段前缀、资源 wire、引用挂载与防重复语义，并建立固定提交差分
- [x] 6.29 补齐 capcut-cli `text-anim` 的 CapCut/剪映入场和出场全目录、默认/显式时长、出场锚点、动画容器复用与同类型防重复语义，并建立固定提交差分
- [x] 6.30 补齐 capcut-cli `image-anim` 的固定 starter 覆盖、CapCut/剪映入场/出场/组合全目录、默认/显式时长、出场锚点、动画容器复用与同类型防重复语义，并建立固定提交差分
- [x] 6.31 补齐 capcut-cli `add-sticker` 的默认/具名轨复用、transform、贴纸素材和六类伴随素材引用闭包，并建立固定提交差分
- [x] 6.32 补齐 capcut-cli `text-ranges` 的 UTF-16 区间、稳定排序、基线空隙样式、BOM 文件输入和结构化失败契约，并建立固定提交差分
- [x] 6.33 验证 Rust `completion` 对标 capcut-cli `completions` 的 bash、zsh、fish 确定性原生脚本能力，并建立固定提交命令映射差分
- [x] 6.34 补齐 capcut-cli `enums` 的 CapCut/剪映双命名空间、14 类完整条目、人类表格和上游大输出截断修复，并建立固定提交差分
- [x] 6.35 补齐 capcut-cli `harvest-enums` 的单草稿扫描、库同步和手工登记；保持 plan-first、`--apply` 原子写、ID/slug 去重、危险类别 id-only 与损坏目录 fail-closed，并建立固定提交差分
- [x] 6.36 补齐 capcut-cli `diagnose` 的 canonical/layout/version、标准候选、字节/时间线哈希、镜像分歧、编辑器状态、人类输出和脱敏 bundle，并建立固定提交差分
- [x] 6.37 补齐 capcut-cli `fixture` 的时间线-only bundle、路径/邮件/设备标识脱敏、嵌套 Timelines、mask-keyframe 证据、媒体排除和独立泄漏复检，并建立固定提交差分
- [x] 6.38 补齐 capcut-cli `compile` 的声明式轨道、refs、九类操作、全量预检、JSONL 批量和 `jianying-job/v2` 兼容载荷，并建立固定提交差分
- [x] 6.39 以真实素材触发的 25→30 fps 代理偏差为 RED，修复 `render proxy` 对草稿帧率的忠实输出；用 Job v2、ffprobe 和发布二进制回归 25/30 fps、无效帧率失败关闭及代理证据身份。
- [x] 6.40 以 Vlog 竖屏素材缩放后代理仍有黑边为 RED，修复 `render proxy` 对草稿 `clip.scale` 的忠实输出；用合成颜色抽帧和真实素材 Job v2 做黑盒回归，明确背景填充仍需独立验收。
- [x] 6.41 以真实 revision 4 的 `canvas_blur` 代理仍有黑边为 RED，补齐按片段引用解析的背景模糊填充、引用失败关闭与独立抽帧测试；发布二进制验证后才标记完成。
- [x] 6.11 逐项消除命令矩阵剩余 `partial`：实现完整可观察语义、固定提交黑盒差分和逐命令证据；不得以已有计划路径或结构测试冒充完成

## 7. 写事务、任务、审批与配置

- [x] 7.1 实现 MutationPlan 的 plan、snapshot、work-copy、validate、atomic-commit 三阶段写流程
- [x] 7.2 验证写入失败时源草稿不变、快照可恢复且审计记录包含恢复命令
- [x] 7.3 实现持久任务的 run、batch、serve、list、show、cancel、retry、audit 和 maintenance 契约
- [x] 7.4 用并发、崩溃恢复和数据量测试比较 JSON journal 与 SQLite，并用 ADR 决定首个生产后端
- [x] 7.5 实现批准记录对 command、arguments、cwd/target、任务和有效期的精确绑定
- [x] 7.6 实现 profile 隔离以及 config schema/validate/get/set/patch/unset 和宿主只读模式
- [x] 7.7 实现 stdio MCP server 与工具目录，并验证它与 CLI 调用共享相同 handler 和错误语义
- [x] 7.8 使用官方 Rust SDK 增加 Streamable HTTP 与 SSE 响应模式，并验证远程监听的 token、Host、Origin 和 fail-closed 门禁
- [x] 7.9 修复 Windows 三平台发布时 8 写入者 SQLite WAL 竞争超过 5 秒的失败；保持 revision CAS、有限等待和 256 任务无丢失回归，并用 Windows CI 复验。

## 8. 自主 headless 对标能力

- [x] 8.1 定义 Runtime Adapter 与 Runtime Profile，记录产品、版本、平台、文件身份和可用 capability
- [x] 8.2 实现未知版本 fail-closed 的产品、进程、权限、草稿根和素材可用性探测
- [x] 8.3 实现已有草稿默认复制编辑，并用源/副本全字段差分证明源草稿未变化
- [x] 8.4 实现本机编辑器 start、stop、status 和安全控制，写入时检测编辑器运行并拒绝不安全操作
- [x] 8.5 实现 ASR provider 接口和 queued/running/succeeded/failed/ambiguous 幂等账本
- [x] 8.6 验证相同内容哈希与配置复用结果，付费或不明状态请求不得静默重提
- [x] 8.7 实现显式授权的原生导出任务、进度/结果验证，并严格区分代理渲染与原生导出
- [x] 8.8 使用仅由本项目生成的合成素材完成独立运行验证，并附受限来源隔离审查记录
- [ ] 8.9 获取合法 Windows 测试环境后确定首个支持 Runtime Profile；在此之前 capability 明确报告不支持
  - 本地门禁证据（2026-09-20）：发布制品验证器和独立打包器均强制 `runtime.profile.windows` 保持 `platform=windows`、`status=external_dependency`、`availability=unsupported`；绕过 CI 步骤顺序也不能签发提前升级的 archive。真实 Windows canary 仍未取得，因此任务保持未完成。
- [x] 8.10 定义 TtsProvider、TtsRequest、TtsArtifact、ProviderCapability 和凭据引用契约，覆盖音色、情绪、SSML、时间戳、流式、格式与平台条件
- [x] 8.11 实现 local-command、macOS say、Windows 系统 TTS、本地 HTTP/进程适配器，并为小米 MiMo、火山、阿里百炼、百度、腾讯、MiniMax、智谱 GLM 建立独立 adapter 和离线 fixture 合约测试
- [x] 8.12 为云端 TTS 增加内容哈希幂等账本、费用审批、ambiguous 状态和禁止静默重提门禁；真实付费 canary 必须单独授权
- [x] 8.13 建立 Qwen3-TTS、CosyVoice、GPT-SoVITS、MeloTTS、ChatTTS、F5-TTS、Fish Speech、IndexTTS 的固定来源档案、具名本地 Runtime Profile、代码/模型双许可证门禁和离线合约测试；不得打包或自动下载权重
- [x] 8.14 审计六个既有 Java 音频研究模块，固定无许可证重写边界和协议分类；新增 EmotiVoice、MARS5-TTS Runtime Profile，并将 EdgeTTS、UnifiedTTS、Whisper 分别路由到联网 TTS、付费 TTS、ASR 后续实现
- [x] 8.15 独立实现 ChatTTS-ui 表单/回环 URL codec、EmotiVoice 本地 OpenAI Audio codec 和 EdgeTTS 结构化进程 Provider，并以离线 fixture 与无 shell 注入测试验证
- [x] 8.16 将七个云端 TTS adapter 接入审批后的真实 HTTP 执行器，按厂商实现鉴权、响应解码、原子音频落盘、草稿事务和账本状态迁移；用回环 mock 证明审批前零网络、成功幂等复用、明确失败与 ambiguous 禁止静默重提，真实付费 canary 仍需单独授权
- [x] 8.17 将原生导出任务状态机完整接入 `render native-task` 命令面，覆盖 show、start、单调进度、验证、失败、中断和成功制品复核；黑盒证明输出哈希漂移 fail-closed，且不得把合成 adapter 证据冒充真实剪映导出
- [x] 8.18 将火山引擎云端 TTS 从已废弃 V1 升级到官方 V3 单向 SSE，使用专用 `X-Api-*` 请求头、资源 ID、唯一请求 ID、多帧音频拼接和终止帧门禁；旧 V1 参数 fail closed，并以离线回环黑盒证明无凭据泄露和无付费调用
- [x] 8.19 独立实现官方 whisper.cpp `whisper-cli` 本地 ASR adapter 和 `media transcribe` 命令，绑定 executable/model 哈希、结构化 argv、原子产物、plan 零执行、幂等复用与显式失败重试；不得捆绑或自动下载运行时和模型权重
- [x] 8.20 实现 `runtime discover` 只读发现 macOS/Windows 已知编辑器安装和草稿根，固定 executable 身份与可得版本，区分 `drafts_without_editor`，并始终保持 unverified/禁止自动路由直到真实 profile canary

## 9. 差分、真实宿主与发布验收

- [x] 9.1 建立 unit、differential、structural、app-open、cold-reopen、playback、native-export 分级证据模型
- [x] 9.2 为所有未批准 wire diff 输出最小字段路径、输入 fixture 和可复现命令
- [ ] 9.3 在受支持的 macOS 剪映版本执行创建、已有草稿编辑、冷重开、播放和原生导出 canary
  - 2026-09-21 真实 canary：Rust 生成的 6 秒合成草稿已通过剪映打开、完整退出后的冷重开、播放和 GUI 原生 MOV 导出；H.264 720p30 + AAC、完整解码和字幕烧录通过。应用保存后的元数据为不透明非 JSON，隔离 `job edit` 失败，新增复制前结构化拒绝，因此整体任务保持未完成。见 `docs/native-canary-20260921.md`，不将 GUI 导出冒充 Rust native adapter。
- [x] 9.4 在 Codex、ZCode、Kimi 经插件执行相同 Job，验证 schema、task ID 和结果语义一致
- [x] 9.5 构建 macOS arm64/x64 与 Windows x64 发布制品、checksums、SBOM、许可证和 capability manifest
  - 正式发布证据（2026-09-21）：GitHub Actions run `35534748729` 从 tag `v1.6.13` 和 source commit `c9ba52ad8a359b6938275438c1e41a11b31f1765` 完成 source/parity gate 及 macOS arm64、macOS x64、Windows x64 三个平台原生测试、release build、运行时命令面验证、SPDX SBOM、确定性打包和上传。稳定 Release 已公开且 `isImmutable=true`，10 个资产均有 GitHub SHA-256；release index SHA-256 为 `92681d802e1362945d95ab9643d92d44d7060a136718a0f686f204b869e7a497`，`gh release verify` 与 `gh release verify-asset` 均通过。
  - 本地门禁证据（2026-09-21）：packager 与 index 聚合器已分别锁定 Windows unsupported profile、正典仓库、三平台精确 archive 名称、released 身份和四重摘要；本机 arm64 `working_tree_unreleased` canary 通过。release workflow 现会在昂贵构建前生成并始终上传结构化远端预检；手工预演只记录，tag 路径严格要求仓库 Immutable Releases 设置，publish 阶段仍二次校验并只下载三个精确平台 artifact，预检证据不会混入 `dist`。当前环境使用已有 Zig、LLVM `dlltool/ar` 和官方 Windows GNU Rust target 已编译全部项目/第三方 crate，仅最终链接因 Zig 0.16 无可用 `msvcrt` import library 失败；临时包装器已删除，未安装或写入仓库工具链。真实三平台不可变制品尚未取得，任务保持未完成。
  - SBOM 与可恢复归档门禁（2026-09-21）：修复了生成器只从 `cargo metadata` 读取 checksum、导致 registry checksum 全部缺失的问题；现在从 `Cargo.lock` 合并 checksum，打包器反向验证完整 236 包名称/版本集合、来源、checksum、唯一 SPDXID 和许可证。截断到单一根包的语法有效 SBOM、缺包、来源漂移和 checksum 篡改均有失败测试；225 个 registry checksum 的真实生成/反向验证通过。SBOM 与 tar.gz/zip 均使用 `SOURCE_DATE_EPOCH` 或 Git 提交时间，固定成员顺序、身份、权限和压缩 header；实际 arm64 连续两次完整打包的 archive、release-entry、checksum sidecar 逐字节相同，archive SHA-256 均为 `21b4beed9b7ccb614905302d80c771126d7af3e0039276fe6a4c2d8a76d62f38`。真实三平台制品仍未取得，任务保持未完成。
  - 隔离 released 演练（2026-09-21）：当前候选复制到临时 Git 仓后以本地 `v1.6.0` tag 和隔离提交 `18f2fc2632c2044987eb89f3863a6aa20507de20` 重编译；二进制、capability manifest、release-entry 均绑定 `released` 身份。两次完整打包逐字节一致，archive SHA-256 均为 `7f206eb0172c3550f766cd13814d580bddeb861ecfa5887de26f0e63e72f2deb`；插件 fetcher/bundle installer 在无 Cargo PATH 下 2/2 通过。该本地演练不替代三平台 GitHub 不可变制品，任务保持未完成。
  - Draft 恢复门禁（2026-09-21）：正式 publish 现可区分不存在、部分 Draft、完整 Draft和已发布不可变 Release；部分 Draft 仅在已有资产按名称、状态、长度和 GitHub digest 构成本次构建的逐字节相同子集时上传缺失文件，额外或漂移资产在公开前 fail closed。离线状态机测试覆盖上述恢复路径且工作流仍禁止 `--clobber`；真实三平台 Release 尚未取得，任务保持未完成。
  - 发布竞态门禁（2026-09-21）：同一 workflow/ref 现以 `cancel-in-progress=false` 串行运行；所有需要公开 Draft 的路径在 `gh release edit --draft=false` 前重新读取远端 assets 并复用状态机验证完整集合，避免完整 Draft 快路径绕过最终复核。真实三平台 Release 尚未取得，任务保持未完成。
  - 防篡改与证明门禁（2026-09-21）：远端预检现要求覆盖 `refs/tags/v*` 的 active、无 bypass、禁止删除和非快进更新的 tag ruleset；Release 查询改为 REST 且仅明确 HTTP 404 表示不存在，权限、限流、网络和服务端错误均 fail closed。公开后重新要求 publication plan 返回 `verify_existing`，并用完整 release attestation 同时绑定远端 tag 对象和全部资产 SHA-256；annotated tag 的 peeled commit 仍由 publication plan 独立绑定 checkout。当前远端未配置所需 ruleset，任务保持未完成。
  - Tag 绑定门禁（2026-09-21）：鉴于 GitHub 只在公开后锁定 Release tag，初次与最终 publication plan 均重新解析远端 lightweight/annotated tag，并要求直接或 peeled commit 等于发布 checkout；tag 删除、移动、重复或异常输出均 fail closed。真实三平台 Release 尚未取得，任务保持未完成。
- [x] 9.6 完成旧命令兼容期文档、迁移指南和回滚演练，不删除 v1 parser
- [ ] 9.7 运行完整 parity、来源、差分和真实剪映门禁，仅在全部目标证据满足后标记变更可归档
