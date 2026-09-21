# jianying 智能体接口契约（Agent Interface）

本文件是智能体调用 `jianying` 的**规范事实源**。命令面不精简、行为不隐含——
每条命令、每个字段、每个退出码都在此定义。变更需同步本文件与 `--help`。

## 1. 进程级契约

| 维度 | 规定 |
|---|---|
| 成功 | stdout **恰好一个** pretty-printed JSON 对象；退出码 0 |
| 运行错误 | stderr 一行 `error: <原因链>`（anyhow 链）；退出码 1 |
| 用法错误 | clap 用法文本到 stderr；退出码 2 |
| 幂等/安全 | `project build` 拒绝已存在输出目录；`store publish` 拒绝同名草稿并备份 root_meta_info；`remove` 必须 `--yes`；`project verify/inspect`、`media catalog`、`doctor` 纯只读 |
| 时间语义 | 一切 `*_us` 字段接受微秒整数或 `tim()` 字符串（`"1h2m3s"` / `"0.5s"` / `"500ms"` / `"1500us"` / SRT `HH:MM:SS,mmm`） |
| 能力命名 | 大小写敏感，必须命中内置目录；用 `jianying media catalog` 检索可用名 |
| VIP 边界 | 目录条目默认过滤 VIP；计划 `allow_vip: true` 才放行（会员权益是用户自己的授权） |

## 2. 命令面（12 组）

### `jianying doctor`
只读环境预检。输出 `{version, plan_schema, draft_roots[], editors_running[], ffprobe?, ffmpeg?}`。
草稿根不存在时给出可操作指引；`editors_running` 非空意味着 publish 会被拒绝。
诊断路径会把用户主目录折叠为 `$HOME`，其他绝对路径只保留文件名；凭据字段固定输出
`[REDACTED]`，不会回显 token、密码或 API key。

### `jianying status [--state-root <dir>]`

汇总 doctor、capability 支持数与持久任务的 total/running/failed，输出整体
`ready|degraded`。远程认证只报告 `configured|missing`，不读取到输出中；任务数据库路径
遵循与 doctor 相同的脱敏规则。

### `jianying audit [<task-id>] [--state-root <dir>]`

带 task id 时返回该任务的追加式状态历史；省略时返回本机任务审计摘要，不输出 job/output
原始绝对路径。该命令只读，不执行重试、取消或清理。

### `jianying completion <bash|elvish|fish|powershell|zsh>`

向 stdout 生成对应 shell 的补全脚本，不安装文件、不修改 shell 配置。配合 `--json` 时，
脚本文本放在标准成功信封的 `data.script` 中。

### `jianying media probe <media>`
ffprobe 实测：`{path, duration_us, width, height, has_video, has_audio, frame_rate, streams, is_image}`。
`frame_rate` 保留有理数分子/分母，`streams` 明确列出 `video` / `audio`，供发布二进制调用方
在不读取 Rust 源码时验证可解码性、帧率和音视频流。
图片（is_image）按 pyJYD 口径计 3 小时名义时长并落 `type:"photo"`。
**规则：计划里所有 duration 必须来自 probe 实测，不许用设计值。**

### `jianying project build <plan.json> --out <dir> [--srt <s.srt>] [--seed <草稿根>] [--template <草稿>]`
计划 → 草稿目录；产物 `draft_content.json` + `draft_info.json`（逐字节相同双镜像）
+ `draft_meta_info.json`（含 draft_materials 注册）+ `assets/`。构建后自动 `verify`，
失败即整体报错不产出。`--srt` 追加字幕轨（`--srt-offset/--srt-size/--srt-align/--srt-color/--srt-border/--srt-y` 全参数面，pyJYD import_srt 线型：type=subtitle/默认 size 5/y -0.8/content 无描边）；`--seed` 从草稿根最新 app 亲写草稿拷贝
schema 标记（防新版 CapCut 拒开）；`--template` 在模板时间线上叠加计划轨道。

### `jianying project verify <dir>` / `jianying project inspect <dir>`
结构 lint（引用完整/主轨连续/时长一致/悬空 ref）与摘要。均只读。

`jianying project diagnose <dir> [--bundle <report.json>]` 进一步检查四个标准草稿同级文件、
canonical/layout/version、字节与时间线哈希、镜像分歧、嵌套 Timelines 线索和编辑器进程；
`--human` 输出紧凑候选表。可选 bundle 原子写入独立路径，不修改草稿，并省略 bundle 自身路径。
正常与分歧草稿均已和固定 capcut-cli 上游完成候选级黑盒差分。

`jianying project fixture <draft> --out <bundle> [--check]` 只复制标准时间线文本和嵌套
`Timelines/` JSON，排除 `assets/` 媒体，并脱敏 Windows/macOS/Linux 用户目录、邮件和
`device_id/mac_address/hard_disk_id`。bundle 同时包含诊断、mask-keyframe 结构报告、说明和
脱敏清单；源草稿保持逐字节不变。`jianying project fixture <bundle> --check` 可独立复检，
发现项只返回文件、行号和类型，不回显敏感内容，并以结构化非零失败阻止分享。

`jianying project compile <spec.json> [--out <draft>] [--drafts <root>] [--check|--plan]`
声明式构建 video/audio/text 轨道，并以条目 `ref` 串接 transition、filter、effect、keyframe、
audio-fade、text-style、text-ranges、template 和 captions 九类操作。相对媒体、字幕和模板路径
以 spec 文件目录解析；`--check`/`--plan` 执行完整预检但不写盘。`--data <rows.jsonl|->`
对字符串中的 `{{key}}` 做标量替换并按行生成独立草稿，必须配合 `--drafts`；默认任一预检失败
即零写入终止，`--continue-on-error` 构建合法行并以 `compile_batch_partial` 返回逐行结果。
单稿始终在同目录随机 staging 中构建、验证，再 rename 提交。相同载荷可放入
`jianying-job/v2.compatibility`，schema 为 `capcut-cli-compile/v1`，由 `job run --out` 执行。
固定上游的预检、九类操作、JSONL 和 fail-fast 差分见
`provenance/CAPCUT_UTILITY_DIFFERENTIALS.json`。

### `jianying project init|quickstart|info|version|diff|describe|migrate|concat`

- `init <name> --out <dir>` 创建可打开的空草稿；`quickstart <name> --out <dir>` 至少接收
  `--video/--audio/--srt` 之一并完成构建与 lint。二者支持画布尺寸、FPS 和 `--seed`。
- `info <dir>` 返回与固定 capcut-cli 基线完全相等的工程、轨道、片段和素材摘要。
- `version <dir>` 只报告实际存在的 timeline/meta 版本标记；当前 Runtime Profile 尚未冻结，
  因此固定 `support.status: untested`、`support.write_guard: block`，不猜测写兼容性。
- `diff <left> <right>` 按片段、素材和轨道输出结构差分，与固定上游输出完全相等。
- `describe` 返回与 `jianying commands` 相同的完整机器命令目录，供智能体发现参数与替代命令。
- `migrate <dir> --from <ver> --to <ver>` 迁移 9.6 mask 字段边界；也支持
  `--like <donor>` 和 `--from-store` 的 schema marker restamp。写入经过 MutationPlan
  的 snapshot、隔离 work-copy、完整校验和同文件系统 atomic commit。
- `concat <left> <right> [--out <dir>]` 将右侧时间线平移后追加，碰撞的素材/片段 ID
  会重建并同步引用；省略 `--out` 时使用同一 MutationPlan 原子提交。事务目录
  `.jianying-transactions/<id>/` 保留原始 snapshot 和 `audit.json`，审计事件包含可执行的
  `jianying store restore-snapshot --snapshot <dir> --target <dir>` 恢复命令。
- `prune <dir> [--dry-run]` 以所有 surviving segment 的 `material_id` 和
  `extra_material_refs` 为引用闭包，保守删除有 ID 的孤儿素材；没有字符串 ID 的素材条目
  始终保留，并返回逐素材类型的 removed/kept 统计。
- `add-cover <dir> <image> [--time <ms>]` 在图片存在后事务化替换顶层 `cover`，
  同时写入 `time`/`time_ms`、图片类型和新的 `custom_cover_id`；时间默认为 0ms。

黑盒证据由 `tools/capcut_project_differential.py` 对固定提交 `49f70e3b` 实际运行，
结果固化在 `provenance/CAPCUT_PROJECT_DIFFERENTIALS.json`；CI 会重新构建上游并逐字比较报告。

### `jianying timeline <op>`

只读查询包括 `tracks`、`segments [--track <type>]`、`get <segment-id>` 和
`quantization-report <start> <duration> [--fps-numerator N] [--fps-denominator D]
[--maximum-drift TIME]`。量化报告使用有理数帧率向外对齐区间，返回量化前后整数微秒及首尾漂移，
任何一端超过显式最大漂移时拒绝，不修改草稿。事务化编辑包括
`add-track`、`add-segment`、`set`、`move`、`move-all`、`trim`、`split`、`speed`、
`volume`、`opacity`、`crop`、`keyframe`、`transition`、`duplicate`、`remove`、`matting`、`chroma`、`mask`、`bg-blur`、`audio-fade`、`add-filter` 和 `composite`。`matting` 在视频素材
对象上切换智能人像抠像，保留应用缓存和未知字段，并报告共享该素材的其他片段。`add-segment` 接收完整无损
segment JSON，材料引用必须在草稿中已存在；提交前由引用闭包校验拒绝悬空引用。
所有写命令都经过 MutationPlan，并会重算顶层 duration。

固定 capcut-cli 差分报告为 `provenance/CAPCUT_TIMELINE_DIFFERENTIALS.json`：`tracks`、
`segments`、`segment`、`shift`、`shift-all`、`speed`、`volume`、`trim` 和 `matting` 黑盒结果完全相等；
`chroma` 的参数钳制、素材结构、引用挂载和定向移除与固定上游语义等价；
`mask` 支持 CapCut/JianYing 资源、几何参数和三种历史素材字段，并跨字段关闭；
`bg-blur` 支持四级画布模糊、替换既有 canvas 引用和关闭；
`audio-fade` 支持 ID 前缀、秒到微秒转换，并在重复应用时替换当前 fade 引用；
`add-filter` 支持固定 CapCut 目录、JianYing 显示名、原始资源 ID、强度、命名轨道和全时间线范围；
`add-effect` 支持固定 CapCut 目录、JianYing 场景/人物特效、原始资源 ID、参数、强度、
片段 ID 前缀绑定、命名轨道和全时间线范围；
`crop` 支持只读查询、归一化矩形、居中比例裁剪、重置、ID 前缀和不落盘 dry-run，
并在素材包含 `crop_ratio` 时按固定上游写回 `free`；
`keyframe` 支持规范属性及 `scale/x/y/opacity` 别名、单点或 stdin JSONL 批量、线性、
ease-in/ease-out/ease-in-out 贝塞尔句柄和按帧模拟的 hold；
`transition` 使用锁定来源的 116 项 CapCut 目录或 pyJianYingDraft 剪映目录，支持默认/显式
时长、Python 成员名、片段 ID 前缀，并拒绝在同一片段堆叠转场；
`image-animation` 使用锁定来源的 CapCut/剪映视频动画全目录和固定上游 starter 覆盖，支持
入场、出场、组合动画的默认/显式时长、片段 ID 前缀、出场尾部锚点和容器复用，并拒绝
同一片段重复同类型动画；
Rust 扩展命令以草稿结构校验和事务恢复证据验收。

`jianying project cut <dir> <start> <end> --out <timeline.json>` 将时间窗提取为独立时间线：
对边界相交片段裁边并重基准到零，按速度同步调整源时间范围，删除空轨和仅由移除片段引用的
孤立素材；源草稿不发生写入。该输出及统计已与固定 capcut-cli 上游逐字段比较。

`jianying captions bubble <dir> <segment-id-or-prefix>` 支持固定气泡 slug 或显式
effect/resource ID，同时写入 `materials.filters` 的 `text_shape` 素材和文字素材的
`bubble_*` 字段；重复设置仅替换当前片段的生效引用，保留旧素材用于无损历史。

`jianying captions animation <dir> <segment-id-or-prefix> <in|out> <animation>` 从锁定来源的
CapCut 或剪映文字入场/出场全目录解析动画，支持默认或显式时长。入场从片段局部时间零开始，
出场锚定到 `segment_duration - animation_duration`；同一文字动画容器可同时持有一项入场和
一项出场动画，但拒绝重复同类型动画。素材 wire、引用和容器复用已与固定 capcut-cli 基线差分。

`jianying captions style-ranges <dir> <segment-id-or-prefix> --styles <json|@file>` 以 UTF-16
码元的 `[start,end)` 区间替换多段文字样式，支持字号、颜色、透明度、粗体、斜体和下划线。
区间按起点稳定排序，未覆盖空隙继承原首样式；BOM 文件可直接读取，重叠、越界、非整数和空数组
在事务提交前被拒绝。输出及 `materials.texts[].content` 已与固定 capcut-cli 基线完全差分。

`jianying completion <bash|zsh|fish>` 只把当前 Rust CLI 的原生补全脚本写到 stdout，不安装、
覆盖或修改 shell 配置。三种 shell 输出均做确定性重复执行验证，并与固定 capcut-cli 的对应
`completions` 能力完成命令映射黑盒验收。

`jianying media enums <category> --namespace <capcut|jianying>` 返回固定 MIT 上游的完整枚举
条目和顺序；支持 transitions、masks、图片/文字动画、场景/人物/音频效果、fonts、filters、
bubbles，以及 `--human` 表格。28 个命名空间/类别组合已逐项验证；固定上游在三个剪映大类别
存在 stdout 65,536 字节截断，Rust 保持完整 JSON，并改用同一固定提交源表完成逐项差分。

`jianying media harvest-enums scan|sync|add` 从单草稿、草稿库或人工证据采集未知资源 ID。
三条路径均默认只输出计划，只有 `--apply` 才以 `0600` 临时文件原子替换用户目录；完整内置 ID、
用户 ID、同草稿和跨草稿候选均参与去重，损坏草稿在 sync 中进入 skipped，损坏目录则拒绝覆盖。
animations、bubbles、fonts 保持 id-only；手工登记还拒绝非规范 ASCII kebab slug 和危险类别。
scan/add/sync 的 plan 与 apply 均已和固定 capcut-cli 上游完成语义差分。

### `jianying media scenes|silence|retakes|evidence`

- `scenes <video>` 使用 ffmpeg scene filter 输出切点和连续分段，支持 `--threshold`、
  `--min-gap`、`--limit` 和 `--ffmpeg-cmd`。
- `silence <media>` 使用 ffmpeg silencedetect 输出静音段与互补保留段，支持
  `--threshold-db`、`--min-silence`、`--pad`、`--limit` 和 `--ffmpeg-cmd`。
- `retakes [<draft> | --srt <file>]` 在限定时间窗口中按词序列 LCS 相似度识别重拍，
  将早期尝试输出为 cut spans，默认保留后一次；草稿模式可用 `--track-name` 限定文本轨。
- `evidence <media>` 生成 `jianying-media-evidence/v1`：把源文件 SHA-256、I 帧时间码、
  静音区间、综合响度、真峰值和音量峰值绑定到同一份只读证据。无视频或无音轨明确记录为
  `present=false`；这些技术事实不能单独升级为叙事或主观质量通过。

四项都是只读分析，不修改草稿。固定 capcut-cli 差分报告
`provenance/CAPCUT_MEDIA_ANALYSIS_DIFFERENTIALS.json` 用相同伪 ffmpeg 和 SRT 输入验证三项
JSON 结果完全相等。

### `jianying media transcribe`

`transcribe <source> --out <artifact> --executable <absolute-whisper-cli> --model
<absolute-model> --model-id <id> --task-id <id>` 通过官方 whisper.cpp `whisper-cli` 做本地转录。
支持 `--format json|verbose-json|text|srt|vtt`、`--language`、`--translate` 和仅用于
`verbose-json` 的 `--segment-timestamps`。调用必须显式提供已审查的本地可执行文件和模型；CLI
不下载或捆绑第三方二进制及权重。

`--plan` 校验执行器/模型身份和请求，返回内容哈希幂等键，但不执行进程、不创建账本、不写
产物。实际执行使用 `--state-root`（默认 `JIANYING_ASR_STATE_ROOT` 或 `.jianying-asr`）保存
queued/running/succeeded/failed 状态；成功且制品哈希未漂移的重复请求直接复用，明确失败只有
增加 `--retry` 才再次启动。命令不经 shell，输出先写 partial 文件并在非空验证后原子提交。
本地真实模型 canary 尚未完成，因此通用 ASR Provider 仍为 partial；具体结构化 adapter
`media.asr_whisper_cpp` 由离线进程黑盒证据支持。

### `jianying media add-video|add-audio|replace|relink|tts|sfx`

- `add-video/add-audio <draft> <source> <start> [duration]` 先用 ffprobe 探测，再把素材复制进
  事务工作副本的 `assets/`，校验通过后原子提交；不会让草稿继续依赖外部绝对路径。
- `replace <draft> <segment-id> <source> [--retime]` 保留片段位置、效果和引用，替换底层素材；
  `--retime` 同步该片段的 source range。
- `relink <draft> (--dir <dir> | --from <old> --to <new>) [--stage]` 修复断链；`--stage`
  将命中的文件纳入草稿事务和素材注册。
- `tts <draft> [start] [duration] (--text <s> | --text-file <f>)` 通过 `--provider` 选择
  `local-command`（默认）、`system-macos`、`system-windows`、`local-http`、`local-process`
  或 `edge-tts`；云端计划还接受 `xiaomi-mimo`、`volcengine`、`aliyun-bailian`、`baidu`、
  `tencent-cloud`、`minimax` 和 `zhipu-glm`。所有 Provider 先逐项校验格式、平台、音色、模型、语言、情绪、语速、音量、
  音高和输入类型，再在同一个 MutationPlan 中合成、ffprobe 验证并加入草稿音频轨。
  `local-command` 使用 `--tts-cmd`，模板必须包含 `{out}`；含 `{text}` 时文本作为单个 argv，
  否则写 stdin。`local-process` 使用绝对 `--executable` 与可重复 `--provider-arg`；
  `local-http` 只接受回环 `--endpoint`；EdgeTTS 明确保持联网属性。命令均不经 shell，
  不会隐式选择云服务、切换 Provider 或触发付费操作。
- 底层统一 `TtsProvider` 契约还包含小米 MiMo、火山引擎 V3 SSE、阿里百炼、
  百度智能云、腾讯云、MiniMax T2A V2、智谱 GLM-TTS 的离线协议 codec。百度的成功响应和
  智谱非流式响应按二进制音频处理，其余 adapter 按各自 Base64、hex 或 URL 契约解码，禁止
  伪装成单一 JSON 协议。火山使用官方 V3 单向 SSE endpoint，调用方必须显式提供
  `--resource-id` 与 `--uid`；执行器写入 `X-Api-Key`、`X-Api-Resource-Id` 和唯一
  `X-Api-Request-Id`，只在收到终止帧后拼接 Base64 音频。旧 V1 `--app-id/--cluster`
  会结构化拒绝，不会静默回退。
  云端 provider 使用 `--credential-env` 引用环境变量名，秘密值不得进入参数、Job 或输出；
  `--task-id`、`--max-cost-microunits` 与 `--plan` 会在不读取秘密、不联网、不修改草稿的情况下
  输出精确审批绑定。`--live-canary` 生成独立的 `tts.cloud.live-canary` 绑定，不能复用生产审批。
  云端提交必须先以 `--plan` 取得绑定，再使用 `--approval-id` 执行；没有匹配审批会在任何网络
  动作前失败。执行器按七家厂商分别处理 Bearer/OAuth/TC3 鉴权、JSON/form 请求以及
  Base64/hex/URL/二进制响应，音频原子落盘后才进入草稿事务。本地和系统 Provider 的 CLI 路由
  不改变这条费用边界。
- 云端提交门禁已由 `TtsLedgerStore` 实现：内容/请求/目标生成幂等键，审批精确绑定 provider、
  model、voice、文本哈希和最大预算；并发 claim、`ambiguous` 对账、显式失败重试及成功制品
  SHA-256 复用规则见 `docs/tts-ledger.md`。回环黑盒测试证明审批消费、单次提交、成功制品复用、
  明确失败和 ambiguous 隔离；`--cloud-endpoint-override` 只接受回环地址。由于尚无七家真实账号
  的独立授权 canary，`media.tts_provider` 继续保持 `partial`。
- `sfx <draft> <slug> <start> <duration>` 使用固定 MIT 上游提交中归档的 CapCut SFX 元数据，
  新建原生 `sound_effect` 素材及 audio 轨片段。
- `add-sticker <draft> <resource-id> <start> <duration>` 新建或复用具名 sticker 轨，支持位置、
  等比缩放和旋转，并按固定 capcut-cli 语义写入 sticker 素材及 speed、placeholder、声道映射、
  人声分离、canvas、material color 六类伴随素材引用。

所有写操作经过 `MutationPlan` 的 snapshot、隔离 work-copy、双镜像与素材注册校验、原子提交。
固定上游黑盒报告为 `provenance/CAPCUT_MEDIA_MUTATION_DIFFERENTIALS.json`；本地合约测试还会
生成真实 MP4/WAV，执行 local-command、结构化 local-process 和当前平台系统 TTS Provider，
验证非回环 local-http 在写草稿前失败，并重新运行 `project verify`。

### `jianying runtime discover|probe|status|start|stop`

`runtime discover` 只读扫描当前平台的已知应用目录和草稿根，也可用重复 `--search-root` 指定
额外应用目录。输出 executable 路径、内容 SHA-256、字节长度、可得版本和草稿根状态；发现草稿
但没有应用时返回 `drafts_without_editor`。所有发现结果均为 `unverified` 且
`automatic_routing=false`，必须先生成并人工审查精确 Runtime Profile，再用 `runtime probe`
校验版本、文件身份、草稿根和 capability。`start/stop` 只控制由本 CLI 以该 profile 启动并
持久记录的进程，不按名称终止其他编辑器进程。

### `jianying store publish <dir> [--root <dir>] [--force]`
拷入草稿库 + root_meta_info 注册 + `.bak` 备份。剪映/CapCut 运行中拒绝（`--force` 覆盖）；
同名拒绝（换名重建，绝不覆盖用户草稿）。

### `jianying render proxy <dir> --out <mp4> [--scale 0.5] [--burn-captions] [--crf 28]`
ffmpeg 代理预览：平铺主视频轨 + 混音全部音频轨（含淡入淡出）+ 可选烧字幕。
**不含转场/特效/蒙版**——权威出口是剪映内导出。
批量代理渲染和 Job JSONL 服务见 [`render-batch.md`](render-batch.md)。

### `jianying render native` 与 `render native-task`

`render native --plan` 生成绑定 Runtime Profile、编辑器身份、草稿摘要、输出和任务 ID 的精确审批；
审批后创建 queued 任务。第一方 adapter 只通过 `render native-task` 上报 start、单调 progress、
interrupt/fail，并在输出完成后调用 verify。verify 会生成 Native 类型、长度和 SHA-256 证据，
result 会再次校验磁盘制品。该命令面完成任务治理闭环，不会把尚未完成的真实剪映 adapter 或
合成输出宣称为 `native-export` 宿主证据。

### `jianying media catalog [--domain <d>] [--search <子串>] [--include-vip]`
16 域能力目录检索。无参数列出各域规模；`--domain` 列条目（默认滤 VIP）；
`--search` 按显示名子串过滤。智能体写计划前应先查名。

### `jianying template <op>`
`list/save/apply/make-preset/apply-preset`（模板库与文字样式预设，详见
[`template-library.md`](template-library.md)）/ `inspect`（轨道+材料清单）/
`duplicate <src> <新名>` / `replace-text <dir> --track <t>
--index <i> <文本>` / `replace-material <dir> <新素材> (--name <n> | --track <t> --index <i>)`
/ `import-track <目标> <源> <轨名> [--before <锚轨>]`（--before=插到锚轨之前，pyJYD insert_track 语义）。对齐 pyJYD 模板模式；replace 后回写并保持
段 source 范围钳制。

### `jianying store <op>`
`list` / `has <名>` / `remove <名> --yes`（删除草稿目录并从 root_meta_info 注销）。

### 兼容别名

旧顶层 `build/verify/inspect/probe/catalog/publish` 和旧式 `render <dir>` 仍调用与上述
规范命令相同的 handler，因此 stdout、退出码和副作用保持一致；它们只在 stderr 增加
`warning: deprecated`。智能体应读取 `jianying commands --json` 中的 `deprecated` 与
`replacement` 字段并迁移到分组命令。

## 3. 计划契约 `jianying-cli-plan/v1`

字段手册见 [`plan-format.md`](plan-format.md)。硬校验（违规即 build 报错）：
主视频轨（如存在）必须第一轨且首段 0 起连续；同轨递增不重叠；fps∈{24,25,30,50,60}；
speed 0.1-8、volume 0-4；关键帧线性、首点 at_us=0、≥2 点（关键帧优先于静态值；贴纸段同视频通道）；转场仅视频轨非末段、时长>0（省略取目录默认）；蒙版/滤镜/特效/动画名须命中目录且
参数 0-100；文本 styles 区间按 UTF-16 升序不越界。

## 4. 双源一致性（维护者）

`tools/parity_run.py`：55 场景（20 个组合动作 + 逐域多样性）同一计划过 pyJYD 与 jianying-cli
语义比对，30/30 必须保持全绿。`tools/gen_catalogs.py --check`：目录与 pyJYD
metadata 一致性校验。两者都在 CI 强制执行。
