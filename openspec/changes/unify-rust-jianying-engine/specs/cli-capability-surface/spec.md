## Purpose

提供面向人类和智能体的稳定命令面，以领域分组覆盖 capcut-cli 固定基线的全部可观察能力，同时维持统一机器输出、配置和任务管理契约。

## ADDED Requirements

### Requirement: 全能力映射
系统 SHALL 为 capcut-cli `49f70e3b` 的 86 个命令逐项维护能力映射，每项必须落入“已支持、部分支持、外部依赖、明确不适用”之一，不能因命令改名而丢失能力。

#### Scenario: 发布候选缺少能力
- **WHEN** 发布门禁发现任一基线命令没有映射或验收状态
- **THEN** 发布失败并列出缺失的上游命令和预期 Rust 命令组

### Requirement: 领域命令树
系统 SHALL 使用稳定领域组组织命令，覆盖项目、时间线、素材、字幕、模板、草稿库、渲染、运行时、任务、配置、诊断和 MCP，而非无限扩展顶层扁平命令。

#### Scenario: 智能体发现命令
- **WHEN** 调用方请求命令或 capability 描述
- **THEN** 系统返回包含参数 Schema、读写级别和输出 Schema 的机器可读目录

### Requirement: 孤儿素材清扫
系统 SHALL 按所有 surviving segment 的主素材和伴随素材引用清扫有 ID 的孤儿素材，保留无字符串 ID 的未知条目，并支持不写入源草稿的预览。

#### Scenario: 预览并清扫孤儿素材
- **WHEN** 草稿同时包含主素材、`extra_material_refs` 伴随素材、孤儿素材和无 ID 未知素材
- **THEN** `project prune --dry-run` 返回与实际清扫一致的逐类型统计且源草稿逐字节不变；实际执行仅删除孤儿素材

### Requirement: 素材级智能抠像
系统 SHALL 在视频或图片片段的主素材上切换智能人像抠像，并在切换时保留应用生成的缓存、笔刷和未知字段。

#### Scenario: 共享素材切换抠像
- **WHEN** 两个片段共享同一视频素材，调用方启用或关闭其中一个片段的智能抠像
- **THEN** 系统写入固定基线约定的 flag 3 或 flag 0 对象，报告另一个受影响片段，且不清空既有缓存字段

### Requirement: 项目封面
系统 SHALL 在封面图存在且时间点为非负整数毫秒时，以事务化写入替换顶层 `cover` wire，并为每次写入生成新的封面 ID。

#### Scenario: 设置并重置封面时间点
- **WHEN** 调用方以 1500ms 设置封面，随后不传时间参数重新设置
- **THEN** 返回值和 `cover.path/type/time/time_ms` 与固定基线语义等价，第二次时间为 0ms，不存在图片被拒绝且草稿不变

### Requirement: 色度键素材
系统 SHALL 为视频片段创建独立色度键素材并挂入伴随素材引用，按固定基线钳制强度；关闭时仅移除该片段引用的色度键素材。

#### Scenario: 启用并关闭色度键
- **WHEN** 调用方以合法十六进制颜色启用色度键后再关闭
- **THEN** 创建的素材字段、强度、引用关系和关闭后的定向清理与固定 capcut-cli 基线语义等价

### Requirement: 跨版本 mask 素材
系统 SHALL 解析 CapCut 与 JianYing 的 mask 元数据，根据应用来源、版本和已有字段选择 `common_masks`、`common_mask` 或 `masks`，并拒绝一个片段叠加多个 mask。

#### Scenario: 写入并跨字段关闭 mask
- **WHEN** 调用方写入带几何参数的 mask，随后素材被应用或旧工具放入另一种已知 mask 字段并执行关闭
- **THEN** 资源元数据和几何结构与固定基线语义等价，关闭操作从片段引用中识别并移除所有三种字段的 mask

### Requirement: 四级背景模糊
系统 SHALL 将背景模糊等级 1、2、3、4 映射到固定基线的画布模糊值，并在设置或关闭前替换片段的既有 canvas 引用。

#### Scenario: 设置并关闭背景模糊
- **WHEN** 调用方设置任一合法等级后执行 `--off`
- **THEN** canvas 素材 wire 结构、片段引用和关闭输出与固定 capcut-cli 基线语义等价

### Requirement: 音频淡入淡出
系统 SHALL 按音频片段完整 ID 或大小写无关前缀设置淡入淡出，将秒转换为整数微秒，并在重复应用时替换当前生效引用而不堆叠。

#### Scenario: 重复设置音频淡入淡出
- **WHEN** 调用方先设置淡入和淡出，再通过同一 ID 前缀重新设置
- **THEN** 素材 wire 结构与时间转换符合固定基线，片段仅引用最新 fade 素材

### Requirement: 音频素材事务能力
系统 SHALL 仅在本地音频经过媒体探测，并且新增、替换、重链、音量和淡入淡出均通过隔离事务契约时，将聚合 capability `media.audio` 标记为 `supported`；TTS 与 ASR 使用各自独立 capability，不得由该聚合能力代替。

#### Scenario: 新增并重链音频素材
- **WHEN** 调用方把含音频流的本地文件加入隔离草稿，再对其执行替换或重链
- **THEN** 系统原子维护素材文件、音频轨片段、时长、音量和路径引用，失败时不提交 staging；capability manifest 暴露 `media.audio=supported`

### Requirement: 颜色滤镜轨道
系统 SHALL 从具有来源证据的固定目录或显式原始资源 ID 解析滤镜，按显式范围或完整时间线创建 filter 轨道片段和 `video_effects` 素材。

#### Scenario: 目录与原始资源滤镜
- **WHEN** 调用方先以目录 slug 和强度创建滤镜，再以独立 resource/effect ID 创建滤镜，最后以 `--full` 覆盖完整时间线
- **THEN** 素材 wire、范围、source platform 和轨道复用语义与固定 capcut-cli 基线等价，无效强度或孤立 effect ID 被拒绝

### Requirement: 文字气泡
系统 SHALL 按文字片段完整 ID 或大小写无关前缀设置气泡，支持固定目录和显式 effect/resource ID，并同时更新 `filters` 素材、片段引用和文字素材字段。

#### Scenario: 替换文字气泡
- **WHEN** 调用方先用目录 slug 设置气泡，再以显式 ID 替换
- **THEN** 两个历史素材均保留，片段仅引用最新气泡，文字素材 `bubble_*` 字段与最新 ID 一致，非文字片段或不完整参数被拒绝

### Requirement: UTF-16 多段文字样式
系统 SHALL 按文字片段完整 ID 或大小写无关前缀，以 UTF-16 码元 `[start,end)` 区间替换多段文字样式；输入支持内联 JSON 和带 BOM 的 `@file`，区间稳定排序，未覆盖空隙继承原首样式。

#### Scenario: emoji、乱序区间与空隙继承
- **WHEN** 调用方对含非 BMP 字符的文字提交乱序且不连续的合法区间
- **THEN** 系统按 UTF-16 码元校验并排序，补齐继承样式的空隙，生成的 `content.styles` 与固定 capcut-cli 基线完全相等；空数组、非整数、重叠和越界输入在事务提交前失败

### Requirement: 特效轨道
系统 SHALL 从固定 CapCut 目录、JianYing 场景/人物目录或显式原始 ID 创建特效，支持参数、强度、显式/全时间线范围和可选片段绑定。

#### Scenario: 创建并绑定特效
- **WHEN** 调用方以目录 slug、参数、强度和片段 ID 前缀创建特效，再创建原始 ID 与全时间线特效
- **THEN** `video_effects` wire、`adjust_params`、强度、`apply_target_type`、完整绑定 ID 和轨道复用语义与固定基线等价，无效绑定不修改草稿

### Requirement: 视频与图片裁剪
系统 SHALL 按片段完整 ID 或大小写无关前缀读取和修改视频/图片素材裁剪，支持显式归一化矩形、固定宽高比、重置和不落盘 dry-run，并在提交前拒绝越界或非有限值。

#### Scenario: 比例、矩形优先级与重置
- **WHEN** 调用方先以 `9:16` 比例居中裁剪，再同时提供比例与显式矩形，执行 dry-run 后重置
- **THEN** 比例计算、显式矩形优先级、八角点 wire、`crop_ratio` 写回、dry-run 不落盘和重置结果与固定 capcut-cli 基线完全相等

### Requirement: 时间窗草稿提取
系统 SHALL 将给定起止时间窗提取为独立时间线 JSON，不修改来源草稿；相交片段必须裁边并重基准到零，源时间范围必须按片段速度调整，完全在窗外的片段、空轨和确定孤立的素材必须移除。

#### Scenario: 长视频时间窗提取
- **WHEN** 时间窗同时截断头尾片段并排除窗外片段
- **THEN** 输出的片段计数、时间范围、源范围、总时长、轨道和素材闭包与固定 capcut-cli 基线完全相等，来源草稿字节不变

### Requirement: 关键帧编辑
系统 SHALL 按片段完整 ID 或大小写无关前缀写入位置、旋转、缩放、透明度、色彩调整和音量关键帧，接受 `scale/x/y/opacity` 属性别名、单点及 stdin JSONL 批量输入，并支持 linear、ease-in、ease-out、ease-in-out 和 hold。

#### Scenario: 批量缓动关键帧
- **WHEN** 调用方混用属性别名、百分比或角度值、全局及逐行缓动写入关键帧
- **THEN** 属性类型、排序、贝塞尔相邻句柄、单点缓动警告和 hold 的前一帧辅助关键帧与固定 capcut-cli 基线语义等价，随机 ID 之外的 wire 完全相等

### Requirement: 双命名空间转场
系统 SHALL 从锁定来源的 CapCut slug 目录或 pyJianYingDraft 剪映目录解析转场，接受片段完整 ID 或大小写无关前缀，采用目录默认时长或显式时长，并拒绝在同一片段堆叠多个转场。

#### Scenario: CapCut 与剪映转场写入
- **WHEN** 调用方分别以 CapCut slug 和剪映 Python 成员名写入默认及显式时长转场
- **THEN** 名称、effect/resource ID、重叠标志、时长、素材 wire 和片段引用与固定 capcut-cli 基线语义等价，随机 ID 之外完全相等

### Requirement: 双命名空间文字动画
系统 SHALL 从锁定来源的 CapCut 或剪映文字入场/出场全目录解析动画，接受文字片段完整 ID 或大小写无关前缀，采用目录默认时长或显式时长，并在同一文字动画容器中最多保留一项入场和一项出场动画。

#### Scenario: 入场与出场动画容器复用
- **WHEN** 调用方分别以 CapCut slug 和剪映 Python 成员名给同一或不同文字片段写入默认及显式时长动画
- **THEN** 入场从局部时间零开始，出场锚定到片段时长减动画时长，素材 wire、引用和容器复用与固定 capcut-cli 基线语义等价，并拒绝重复同类型动画

### Requirement: 视频与图片三类动画
系统 SHALL 从锁定来源的 CapCut/剪映视频动画全目录和固定上游 starter 覆盖中解析入场、出场与组合动画，接受片段完整 ID 或大小写无关前缀，并在同一动画容器中最多保留每种类型一项。

#### Scenario: 入场、出场与组合动画并存
- **WHEN** 调用方同时写入默认或显式时长的入场、出场与组合动画，或用剪映成员名写入全目录动画
- **THEN** 入场和组合动画从局部时间零开始，出场锚定到片段时长减动画时长，目录覆盖、缓存路径、素材 wire、引用和容器复用与固定 capcut-cli 基线语义等价，并拒绝重复同类型动画

### Requirement: 原生贴纸片段
系统 SHALL 通过资源 ID 在默认或具名 sticker 轨上新增贴纸片段，支持开始时间、时长、位置、等比缩放和旋转，并完整注册固定上游所需的伴随素材引用闭包。

#### Scenario: 默认贴纸轨复用
- **WHEN** 调用方连续两次省略轨道名新增不同贴纸
- **THEN** 两个片段复用同一默认 sticker 轨，贴纸素材、transform wire、speed、placeholder、声道映射、人声分离、canvas 和 material color 与固定 capcut-cli 基线语义等价

### Requirement: 素材清单与下钻
系统 SHALL 在 `media` 领域提供与固定 capcut-cli 基线等价的素材类型计数、按类型摘要和按完整 ID/大小写无关前缀读取无损素材对象的只读命令。

#### Scenario: 按类型浏览素材
- **WHEN** 调用方对草稿执行 `media materials --type videos`
- **THEN** 系统按原顺序返回每项的 id、名称、路径、时长、类型和字段数，不修改草稿

#### Scenario: 用 ID 前缀下钻
- **WHEN** 调用方用素材 ID 的大小写无关前缀执行 `media material`
- **THEN** 系统返回带 `_type` 的完整原始素材对象；没有匹配时返回非零结构化失败

### Requirement: OpenTimelineIO 交接
系统 SHALL 将视频、音频剪辑、间隙、源区间、速度、音量和可选字幕标记导出为与固定
capcut-cli 基线等价的 OpenTimelineIO 0.14 稳定 Schema；没有可移植表示的轨道必须显式报告。

#### Scenario: 导出可移植时间线
- **WHEN** 调用方执行 `project export-timeline` 并指定输出文件
- **THEN** 系统写出 Timeline.1/Stack.1/Track.1/Clip.1 文档，返回轨道、片段、间隙、字幕和跳过项统计

#### Scenario: 导入可移植时间线
- **WHEN** 调用方以 `--out` 创建新草稿或以 `--into` 追加到已有草稿
- **THEN** 系统恢复视频、音频、间隙、源区间、速度、音量和字幕 Marker，本地文件进入草稿资产目录，缺失文件成为显式占位素材，所有不支持的 OTIO 特性均被报告

### Requirement: 统一 JSON 输出
系统 SHALL 支持全局 `--json`，成功时 stdout 仅包含一个 JSON 文档，失败时返回统一错误信封并使用非零退出码；进度和诊断只能写入 stderr。

#### Scenario: JSON 模式下构建失败
- **WHEN** 构建命令因素材不存在而失败且启用 `--json`
- **THEN** stdout 仅包含带错误类型、消息和可操作详情的失败信封

### Requirement: 声明式草稿编译
系统 SHALL 通过 `project compile` 接受与固定 capcut-cli 基线语义等价的声明式规范，覆盖
video/audio/text 轨道、条目 refs、transition、filter、effect、keyframe、audio-fade、
text-style、text-ranges、template、captions 九类后处理操作，以及 JSONL 占位符批量构建。
系统 SHALL 在任何单稿写入前完成媒体、操作文件、引用、名称和参数预检；单稿构建 SHALL
在隔离 staging 中完成验证后原子提交。相同规范 SHALL 可作为 `jianying-job/v2` 的
`capcut-cli-compile/v1` compatibility payload 执行。

#### Scenario: 九类操作的声明式成稿
- **WHEN** 调用方提交包含三类轨道、refs 和九类操作的合法规范
- **THEN** 生成草稿的画布、时长、片段计数、ref 解析及每类操作的可观察 wire 语义与固定上游等价，并通过完整 bundle 验证

#### Scenario: 批量预检发现坏行
- **WHEN** JSONL 第二行缺少规范中的占位符且未启用 `--continue-on-error`
- **THEN** 命令返回 1-based 行号和非零结构化失败，所有行在写入前完成预检且草稿目录保持为空

### Requirement: Shell 补全生成
系统 SHALL 为 bash、zsh 和 fish 生成确定性的原生补全脚本，不安装或修改用户的 shell 配置；Rust 命令名可以与固定 capcut-cli 不同，但 SHALL 覆盖自身实际命令面和可执行文件名。

#### Scenario: 三种 shell 补全
- **WHEN** 调用方分别请求 bash、zsh 和 fish 补全
- **THEN** 每次输出均为确定、非空、符合目标 shell 语法且绑定 `jianying` 的脚本，其可观察能力与固定 capcut-cli `completions` 命令语义等价

### Requirement: 双命名空间枚举目录
系统 SHALL 以只读命令列出固定 CapCut 与 JianYing 命名空间的 transitions、masks、图片动画、文字动画、场景/人物/音频效果、fonts、filters 和 bubbles；返回条目、顺序和空类别 SHALL 与固定来源一致，并支持独立的人类表格输出。

#### Scenario: 固定上游大目录输出被截断
- **WHEN** 固定 capcut-cli 的大类别在管道中以成功退出码产生 65,536 字节截断 JSON
- **THEN** Rust 命令仍输出完整有效 JSON，并以同一固定提交的 `src/enums.json` 逐项验证条目和顺序，不得复刻截断缺陷或把计数验证冒充完整差分

### Requirement: 用户枚举采集
系统 SHALL 从单个草稿或草稿库扫描 app-authored 的 effect/resource/font ID，并允许手工登记可安全映射的类别。所有路径 SHALL 默认仅生成计划，只有显式 apply 才原子写入独立用户目录；内置与用户 ID、同草稿和跨草稿候选均须去重，animations、bubbles、fonts 保持 id-only。

#### Scenario: 草稿库混有损坏项目
- **WHEN** 库同步遇到两个有效草稿共享资源且另有一个损坏草稿
- **THEN** 系统按稳定目录顺序扫描有效草稿，共享资源仅生成一次，损坏草稿以结构化原因进入 skipped，plan 不写文件，apply 原子写入；损坏的既有用户目录不得被覆盖

### Requirement: 草稿存储诊断
系统 SHALL 只读检查 `draft_content.json`、`draft_info.json`、`draft_meta_info.json`、`template-2.tmp` 及嵌套 Timelines 证据，报告 canonical、布局、版本、字节哈希、可解析性、轨道/片段摘要、镜像分歧、编辑器状态和可操作建议。可选 bundle SHALL 使用脱敏报告且不得写入源草稿。

#### Scenario: 同级时间线发生分歧
- **WHEN** 两个可读时间线同级文件的语义哈希不同并请求诊断 bundle
- **THEN** 报告 `ok=false`、`diverged=true`，保留每个候选的字节和时间线证据，给出先关闭编辑器并审核同步计划的动作；bundle 原子写入指定路径且不包含自身路径字段

### Requirement: 脱敏兼容性 Fixture
系统 SHALL 从真实草稿只读生成不含媒体的兼容性 bundle，仅复制标准时间线和嵌套
`Timelines/` 文本，脱敏用户目录、邮件和设备标识，并附诊断、mask-keyframe 结构报告、说明及
脱敏清单。系统 SHALL 支持对既有 bundle 独立复检；发现项只能报告文件、行号和类型，不能回显
疑似敏感值。

#### Scenario: Bundle 中残留用户目录
- **WHEN** 独立复检发现文本仍含真实用户目录
- **THEN** 命令以非零结构化失败返回 `home-path` 的文件和行号，不在 stdout/stderr 回显目录值，且不修改来源草稿

### Requirement: 批处理和持久任务
系统 SHALL 同时支持单事务批处理和可恢复后台任务，并提供 list、show、cancel、retry、audit 和 maintenance 行为。

#### Scenario: 长时间原生导出中断
- **WHEN** 导出进程在完成前被中断
- **THEN** 任务记录保留最后检查点、终态原因和安全恢复入口

### Requirement: 配置 profile 与只读模式
系统 SHALL 支持隔离 profile、配置 schema/validate/get/set/patch/unset，以及阻止配置写入的宿主级只读模式。

#### Scenario: 外部管理配置
- **WHEN** 宿主启用配置只读模式后调用写配置命令
- **THEN** 系统拒绝写入但仍允许 schema、validate 和 get

### Requirement: MCP 多 transport 与远程安全边界
系统 SHALL 使用官方 Rust MCP SDK 同时提供 stdio 与 Streamable HTTP 服务端 transport；HTTP 响应 SHALL 支持 JSON 和 SSE 流模式。非回环地址监听必须要求 Bearer token、显式 Host allowlist，并允许配置浏览器 Origin allowlist。系统不得把已废弃的 2024-11-05 独立 HTTP+SSE transport 冒充为现代 Streamable HTTP。

#### Scenario: Pad 通过局域网访问电脑上的剪映工具
- **WHEN** 用户以非回环地址启动 Streamable HTTP 或 SSE 响应模式，并提供 token、Host 与 Origin allowlist
- **THEN** 客户端可在单一 MCP endpoint 完成初始化、通知、工具发现与调用，未授权请求返回 401

#### Scenario: 远程客户端管理持久任务生命周期
- **WHEN** stdio 或远程 MCP 客户端需要观察、取消或审计已提交的剪映任务
- **THEN** 同一工具目录提供 list、show、cancel、retry 和 audit，并复用 CLI 的 SQLite JobStore、状态机和错误语义；连接断开或 JSON-RPC 请求取消不得被伪报为业务副作用已经撤销

#### Scenario: 无凭据远程暴露
- **WHEN** 用户请求监听非回环地址但未配置 Bearer token 或显式 Host allowlist
- **THEN** 服务在绑定端口前 fail closed，并返回可操作的配置错误

### Requirement: 可扩展 TTS Provider 与付费边界
系统 SHALL 将 TTS 建模为独立 Provider 能力，而不是只接受一个固定厂商或把全部厂商伪装成
相同 HTTP API。统一请求至少包含文本、模型、音色、语言、语速、音量、音高、情绪、输出格式、
采样率、流式偏好和可选时间戳；Provider capability SHALL 明确声明其实际支持项、平台条件、
鉴权方式、文本长度和输出模式。首批目标包括小米 MiMo TTS、火山引擎、阿里云百炼
CosyVoice/Qwen TTS、百度智能云、腾讯云、MiniMax、智谱 GLM、macOS `say`、Windows 系统
TTS，以及本地命令和本地 HTTP 模型服务。

本地开源运行时 SHALL 使用具名 Runtime Profile，而不是仅以 `local-http` 或 `local-process`
笼统宣称支持。首批具名档案包括 Qwen3-TTS、CosyVoice、GPT-SoVITS、MeloTTS、EmotiVoice、
MARS5-TTS、ChatTTS、F5-TTS、Fish Speech 和 IndexTTS。每个档案必须分别记录固定上游提交、代码许可证、实际模型
制品许可证结论、传输协议和商用策略；CLI 不得打包或自动下载第三方模型权重。

#### Scenario: 选择云端语音厂商
- **WHEN** 调用方指定一个已配置的云端 Provider、模型与音色
- **THEN** 系统先校验 capability、凭据引用、费用策略和输出格式，再通过该 Provider adapter 合成并把音频纳入同一草稿事务

#### Scenario: 火山引擎 V3 流式响应
- **WHEN** 调用方选择火山引擎并提供 V3 resource ID、uid、音色和 API key 凭据引用
- **THEN** 系统使用官方单向 SSE endpoint 与专用 `X-Api-*` 请求头，按序拼接成功音频帧，并在缺失终止帧、错误帧或旧 V1 配置出现时 fail closed

#### Scenario: 使用系统或本地语音
- **WHEN** 调用方选择 macOS、Windows、本地命令或本地 HTTP Provider
- **THEN** 系统仅在当前平台及可执行文件/endpoint 探测通过时报告 supported，并记录实际使用的 Provider、模型或系统音色

#### Scenario: 宽松代码许可证但模型制品尚未审查
- **WHEN** Qwen3-TTS、CosyVoice、GPT-SoVITS 或 MeloTTS 的代码运行时可用，但所选 checkpoint、声库或附属制品没有固定哈希和许可证审查记录
- **THEN** 系统不得进入商用合成路径，也不得仅凭代码仓库的 Apache/MIT 许可证报告该模型制品可商用

#### Scenario: 本地模型带有非商业或单独授权限制
- **WHEN** 商用 Job 选择 ChatTTS、F5-TTS、Fish Speech、IndexTTS 或其他策略为 non-commercial、permission-required、research-only、unknown 的运行时
- **THEN** 系统在启动本地进程或发送 HTTP 请求前返回结构化许可证拒绝，不得静默降级到其他模型

#### Scenario: 本地运行时网络边界
- **WHEN** 具名本地 TTS Runtime Profile 提供默认 HTTP endpoint
- **THEN** endpoint 默认只绑定回环地址；远程暴露必须复用独立的鉴权、Host 和 Origin allowlist 门禁

#### Scenario: 付费请求缺少授权
- **WHEN** 云端 Provider 可能计费且调用上下文没有匹配的审批或明确费用策略
- **THEN** 系统在网络提交前拒绝请求，不得静默重试、切换厂商或消费其他账号额度

#### Scenario: 相同云端 TTS 请求重复提交
- **WHEN** 相同 Provider 配置、统一请求和目标草稿已经处于 queued、running、failed 或 ambiguous
- **THEN** 普通提交路径拒绝再次调用远端；failed 只能在新审批下显式 retry，ambiguous 必须先对账

#### Scenario: 已成功的云端 TTS 请求复用
- **WHEN** 相同幂等键已有 succeeded 记录且本地音频制品的 SHA-256 仍与账本一致
- **THEN** 系统复用该制品而不消费新审批或调用远端；制品缺失或漂移时 fail closed

#### Scenario: 真实付费 canary
- **WHEN** 调用方请求真实云端 TTS canary
- **THEN** 审批必须显式绑定 `tts.cloud.live-canary`、Provider、模型、音色、文本哈希、目标和预算，普通生产审批不得复用

#### Scenario: Ollama 没有音频输出能力
- **WHEN** 当前 Ollama capability 探测没有原生 TTS/audio-output endpoint
- **THEN** 系统不得将 Ollama 标记为直接 TTS Provider；只能把它作为文本编排器连接到独立本地 TTS 引擎

#### Scenario: 本机客户端实际依赖远程语音服务
- **WHEN** 调用方选择 EdgeTTS 或 UnifiedTTS 等需要外网、账号或远程计算的集成
- **THEN** 系统必须报告联网或付费 Provider，不得因为客户端进程运行在本机就标记为离线本地模型

#### Scenario: 研究实现名称与真实协议不一致
- **WHEN** 来源模块以 MARS5-TTS 命名但实际仍使用 ChatTTS-ui `/tts` schema
- **THEN** 系统拒绝把该 schema 登记为 MARS5 协议，只允许通过独立来源验证的进程或 endpoint adapter

#### Scenario: Whisper 音频研究进入 ASR 边界
- **WHEN** 迁移 Whisper multipart 转录、翻译、SRT/VTT 或 word/segment 时间戳能力
- **THEN** 能力归入 ASR Provider 和幂等账本，不得混入 TTS Runtime Profile

### Requirement: whisper.cpp 本地 ASR Runtime
系统 SHALL 通过官方 whisper.cpp `whisper-cli` 的结构化进程接口提供本地 ASR，执行器身份必须
同时绑定可执行文件与模型内容哈希。系统不得通过 shell 调用，不得自动下载或打包第三方运行时
与模型权重；产物必须先写 partial 路径、验证非空并原子提交。

#### Scenario: ASR 计划零执行
- **WHEN** 调用方使用 `media transcribe --plan` 提供源文件、绝对 executable、绝对 model、model ID 和输出格式
- **THEN** 系统返回 Provider、executor identity、源哈希和幂等键，但不启动进程、不创建账本或产物

#### Scenario: 本地 ASR 幂等复用与失败重试
- **WHEN** 相同请求已成功且产物哈希未漂移
- **THEN** 系统复用账本制品且不再次启动 whisper.cpp；明确失败只有显式 `--retry` 才能再次执行并递增 attempts

#### Scenario: whisper.cpp 参数和产物边界
- **WHEN** 调用方请求 json、verbose-json、text、srt 或 vtt，并可选语言与翻译
- **THEN** 系统使用官方 CLI 参数逐项映射且不经 shell；不兼容的格式/时间戳、源哈希、模型 ID 或既有输出在启动前 fail closed

#### Scenario: 无许可证的内部研究实现
- **WHEN** Java 研究仓库没有可确认的仓库级许可证
- **THEN** 迁移只能独立重写协议事实和测试场景，不得逐文件复制源代码，并在来源清单中记录 `NOASSERTION`
