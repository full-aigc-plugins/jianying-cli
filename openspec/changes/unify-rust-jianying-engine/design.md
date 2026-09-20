## Context

当前仓库是单 crate Rust CLI，`main.rs` 直接分派构建、模板、草稿库、校验和代理渲染，`plan.rs` 以大量可选字段表达所有片段。已有 55 个 pyJianYingDraft 差分场景，但 capcut-cli 的 86 项命令能力、已有草稿广泛编辑、任务执行、ASR 和原生导出尚未进入统一架构。详见 proposal 和四份 capability spec。

命令设计参考 OpenClaw 官方 CLI 的领域命令树、全局 profile、TTY/JSON 双输出、统一 JSON 失败信封、doctor、tasks、approvals、config 和 MCP 分层；只参考交互原则，不复制具体业务命令或实现：<https://docs.openclaw.ai/cli>。

## Goals / Non-Goals

**Goals:**

- 一个 Rust 二进制覆盖创建、编辑、检查、批处理、任务、ASR、代理/原生导出和运行时控制。
- 以强类型领域模型承载 pyJianYingDraft 数据语义、capcut-cli 命令能力和自主 headless 能力。
- 兼容现有 `jianying-cli-plan/v1` 与当前顶层命令，按版本逐步迁移。
- 所有副作用具有事务、快照、审批和审计边界。
- 每项能力具有固定来源、差分测试和真实宿主证据。

**Non-Goals:**

- 不分发剪映程序、官方动态库、账号数据、在线资源、字体、效果包或授权缓存。
- 不复制或翻译非商业 headless 上游的源码、测试、蓝图、资源及实现常量。
- 不保证所有剪映版本立即可用；能力由 Runtime Profile 明确限定。
- 不把 FFmpeg 代理渲染描述为剪映原生渲染。

## Decisions

### 1. 使用 Cargo workspace 隔离领域与适配层

目标布局：

```text
crates/
  jianying-domain/      # 项目、时间、轨道、片段、素材、操作
  jianying-schema/      # Job/Plan 版本、兼容转换、JSON Schema
  jianying-draft/       # 草稿 wire model、镜像、读取/写入
  jianying-store/       # 草稿库、注册、备份、恢复、锁
  jianying-media/       # probe、SRT/ASS、OTIO、FFmpeg 适配
  jianying-runtime/     # 产品探测、进程控制、原生运行时 adapter
  jianying-jobs/        # task、checkpoint、audit、approval
  jianying-mcp/         # stdio / Streamable HTTP / SSE 响应模式 MCP server
  jianying-cli/         # Clap 命令与人类/JSON呈现
```

领域 crate 不依赖 CLI、文件布局或宿主插件。现有模块先通过兼容 facade 迁入对应 crate，避免一次性重写。

MCP 服务使用官方 Rust SDK：本机宿主默认走 stdio；远程 Pad 访问走单一 `/mcp`
Streamable HTTP endpoint。`sse` 是 Streamable HTTP 的强制事件流响应 profile，不重新实现
已废弃的 2024-11-05 独立 HTTP+SSE transport。HTTP 默认只绑定回环地址；非回环监听必须
同时提供 Bearer token 与 Host allowlist，浏览器来源可再通过 Origin allowlist 收紧。

替代方案是继续扩大单 crate；拒绝原因是 86 项能力、原生运行时和任务状态会把解析、领域和 IO 耦合在一起。

### 2. 使用 tagged enum 替代万能 Segment

统一模型以 `DraftProject -> Timeline -> Track -> Segment` 为主干。`Segment`、`Material` 和 `EditOperation` 使用 serde tagged enum。所有时间使用 `TimeRange` 和 `FrameRate` 值对象，原生量化产生显式 `QuantizationReport`。

读取已有草稿时同时保存 typed view 与 lossless raw envelope；写回只 patch 已知目标路径，以保留未知字段。新建草稿由 typed model 完整生成。

### 3. Schema 使用统一 Job 信封

新增 `jianying-job/v2`：

```json
{
  "schema": "jianying-job/v2",
  "operation": "create|edit|inspect|verify|publish|export|batch",
  "runtime": {},
  "project": {},
  "assets": [],
  "timeline": {},
  "operations": [],
  "export": null,
  "policy": {}
}
```

`v1` 通过纯转换器进入 `v2`，并有 golden tests。Schema 演进遵循新增兼容字段优先；行为破坏必须提升版本。

### 4. 采用领域命令树，不复制 86 个顶层命令

建议主命令：

```text
jianying
  project   init|quickstart|info|version|diff|concat|migrate|describe
  timeline  show|tracks|segments|get|add|set|trim|shift|speed|volume|...
  media     probe|materials|add|replace|relink|scenes|silence|retakes|tts|sfx
  captions  list|set|import|export|style|caption|translate
  template  list|save|apply|preset|duplicate|import-track
  store     list|register|rename|sync|backup|restore|decrypt|catalogue
  render    proxy|native|batch
  job       run|batch|serve|list|show|cancel|retry|audit|maintenance
  runtime   status|probe|start|stop|export-capabilities
  approvals pending|resolve|grants
  config    file|schema|validate|get|set|patch|unset
  mcp       serve|tools
  doctor
  status
  audit
  completion
```

旧命令保留为输出弃用警告的别名，内部调用同一 command handler。能力矩阵记录每个 capcut-cli 命令落在哪个新命令，并由 CI 验证无空项。

### 5. 全局输出与错误契约

全局选项至少包含 `--json`、`--profile`、`--no-color`、`--log-level`。TTY 模式允许表格与进度；JSON 模式 stdout 只输出一个文档。失败信封统一为：

```json
{"ok":false,"error":{"type":"...","message":"...","details":{},"recovery":[]}}
```

任务型操作额外返回稳定 `task_id`。敏感路径和凭据在诊断中脱敏。

### 6. 写操作使用计划、快照、提交三阶段

所有写操作先产生 `MutationPlan`，再创建只读源快照与目标工作副本，验证后原子提交。草稿库登记、镜像更新与素材复制属于一个可恢复事务。失败保留审计和恢复指令，不自动清理用户可能需要的证据。

### 7. 原生能力使用 adapter 与 Runtime Profile

自主实现的原生层只定义本项目接口：运行时探测、草稿读写 adapter、编辑器控制、导出任务和结果验证。每个支持组合用 Runtime Profile 绑定产品、版本、平台、文件身份和能力；未知组合 fail closed。

基础实现使用内容 SHA-256 绑定可执行文件身份，并把产品、精确版本、平台、进程名、批准的草稿根
和 capability 作为一个不可隐式降级的 profile。已有草稿编辑先复制到全新且不位于源目录内的
副本；复制前后以相对路径、长度和文件 SHA-256 做全树差分。进程控制只终止当前 adapter 自己
启动并持有的子进程，不使用按名称全局 kill。真实剪映 profile 在合法宿主 canary 前仍保持
partial/unsupported，不由合成进程测试提升。

平台发现与支持判定分离：`runtime discover` 只读枚举已知应用目录和草稿根，计算 executable
身份并读取可得版本；即使发现安装也固定输出 `unverified` 和 `automatic_routing=false`。
草稿存在而应用缺失是独立的 `drafts_without_editor` 状态，不能据此生成受支持 profile。

原生导出任务使用独立的持久状态机和 `render.native` 精确审批。提交绑定 profile、可执行文件
SHA-256、草稿全树 SHA-256、输出和覆盖策略；进度只允许单调前进，中断保留检查点和恢复 argv。
输出需经过非空、长度和 SHA-256 验证，覆盖既有输出时内容必须变化。Native 制品类型不能由
Proxy 请求构造。合成测试只证明任务治理，真实宿主导出仍由 9.3 门禁。

不得提交官方二进制或受限资源。对系统/应用内部接口的使用必须经过单独兼容与分发审查。

### 8. ASR/TTS 作为 provider + ledger

CLI 不绑定单一云服务。ASR provider 接收内容哈希、音频事实和 provider 配置，账本状态为 queued/running/succeeded/failed/ambiguous。相同哈希和配置只能有一个有效请求；外部付费提交需要插件或用户授权。

ASR 幂等键由 Provider、含版本/哈希的 executor identity、源内容 SHA-256 和规范化请求共同生成，
不包含音频正文或源路径。首次提交使用跨进程原子 claim；完成结果只有在转录制品存在且哈希一致
时才复用。付费执行使用 `asr.cloud.submit` 精确审批，`ambiguous` 禁止重提，明确失败也只能在
显式 retry 后执行。本地执行强制零预算，避免把本地和付费边界混为一谈。

首个具体 ASR runtime 使用官方 whisper.cpp `whisper-cli` 的结构化 argv adapter。调用方必须
显式提供绝对 executable、绝对 model 路径和逻辑 model ID；executor identity 同时绑定 executable
与模型 SHA-256。CLI 不经 shell、不自动下载或打包运行时/模型，输出通过唯一 partial prefix
生成并在非空验证后原子提交。真实模型 canary 前只把该具体 adapter 的离线契约标为 supported，
通用 ASR Provider 仍保持 partial。

TTS 使用同一治理原则，但保留独立的 `TtsProvider` 契约。统一领域请求不泄漏厂商字段；
Provider adapter 负责鉴权、参数映射、同步/流式/异步响应、Base64/hex/URL/二进制解码、
音频格式归一和 request-id 审计。Provider capability 必须逐项声明 voice catalogue、SSML、
voice clone/design、emotion、timestamps、streaming 和字符上限，不能用统一接口掩盖不支持项。

Provider 分层：

- `local-command`：现有无 shell `--tts-cmd`，作为通用逃生口。
- `system-macos` / `system-windows`：系统音色适配器，平台探测后启用。
- `local-http` / `local-process`：未知或自定义本地服务的通用逃生口，不作为具名模型已支持的证据。
- 具名本地 Runtime Profile：Qwen3-TTS 当前按官方本地 Python/离线进程接入；CosyVoice 按官方
  FastAPI 路由接入；GPT-SoVITS、MeloTTS、EmotiVoice、MARS5-TTS 先建立显式协议档案。只有
  进程/endpoint 握手和离线 fixture 通过后，具体 adapter 才能从 partial 提升为 supported。
- 许可证双门禁：分别记录运行时代码和所选 checkpoint/声库/附属制品的许可证。Qwen3-TTS、
  CosyVoice、GPT-SoVITS、MeloTTS 即使代码为 Apache/MIT，也必须完成具体模型制品审查；
  ChatTTS、F5-TTS 的公开权重按非商业处理，Fish Speech、IndexTTS 按需单独书面授权处理。
- Java 音频研究模块只提供协议事实：研究仓库没有可确认的仓库级许可证，不复制其实现。
  EdgeTTS 是调用微软在线语音服务的本地客户端，不是离线模型；UnifiedTTS 是付费远程聚合器；
  Whisper 进入 ASR Provider。MARS5 Java 示例当前复用 ChatTTS-ui schema，拒绝将该伪协议迁移为
  MARS5 adapter。MARS5 的 AGPL 运行时保持外部进程边界并单独执行 copyleft 合规门禁。
- ChatTTS-ui adapter 仅编码已验证的表单字段并拒绝非回环音频 URL；EmotiVoice adapter 使用回环
  OpenAI Audio JSON/二进制边界；EdgeTTS 使用结构化 argv 和原子文件提交，不使用 shell、不自动
  重试，也不因本机存在可执行文件而隐藏其联网属性。
- CLI 不随包分发或自动下载第三方权重；来源清单固定上游提交，实际安装制品另记路径、哈希和
  模型卡。网络型本地运行时默认仅使用 `127.0.0.1`。
- `xiaomi-mimo`、`volcengine`、`aliyun-bailian`、`baidu`、`tencent-cloud`、`minimax`、
  `zhipu-glm`：显式命名的云端 adapter，各自持有参数映射和错误分类。
- 云端协议不强制伪装成一种响应：火山 V3 使用 SSE Base64 音频分片，MiMo、腾讯使用 Base64
  JSON，MiniMax 使用 hex JSON，阿里使用限时 URL，百度与智谱非流式成功响应使用二进制音频。
  百度 `tok` 和各厂商 Authorization/签名只能由执行器在提交边界注入，凭据引用不得进入协议
  fixture。火山 V3 独立写入 `X-Api-Key`、资源 ID 与唯一请求 ID，收到终止帧前不得提交音频；
  旧 V1 `app-id/cluster` 配置 fail closed。

凭据只允许通过 profile 中的环境变量引用或宿主 secret provider 获取，不进入 Job、日志、审计
正文或命令行。云端请求先写 ledger 和幂等键；超时后进入 `ambiguous`，未确认远端状态前不得
自动重提。费用审批绑定 provider、model、voice、文本哈希、目标草稿和最大预算。
账本的首次提交使用跨进程可见的原子 claim；崩溃遗留 claim 按 `ambiguous` fail closed。
`failed` 仅允许在新审批下显式重试，`succeeded` 仅在制品存在且 SHA-256 匹配时复用。
真实 canary 使用独立的 `tts.cloud.live-canary` 审批命令，不能复用普通生产审批。

Ollama 当前不作为直接 TTS adapter：其公开能力没有稳定的音频输出/TTS endpoint。未来只有在
capability probe 实际发现标准音频输出协议后才能提升状态；在此之前可用 Ollama 生成/润色文本，
再交给独立本地 TTS Provider。

### 9. 差分测试与证据分层

- pyJianYingDraft：同计划双构建，规范化 wire diff。
- capcut-cli：每个命令能力使用共享 fixture 或黑盒命令结果对比。
- 自主 headless 能力：只使用本项目生成的合成素材和独立运行证据。
- 证据等级：unit → differential → structural → app-open → cold-reopen → playback → native-export。

`provenance/EVIDENCE_MODEL.json` 固定七级顺序、每级证明范围和必需制品，单条记录遵循
`schemas/evidence-record-v1.schema.json`。合成进程与代理渲染最高为 structural；app-open 以上
必须绑定具名平台、产品、版本和 runtime identity；native-export 还必须绑定审批和输出哈希。
差分失败使用 RFC 6901 JSON Pointer 输出最小字段路径，并同时保存仓库内 fixture 路径与不经
shell 拼接的复现 argv。已批准差异保留 rationale，未批准差异不能只输出自然语言摘要。

固定 `SOURCE_MANIFEST` 记录来源提交、许可证、用途和文件哈希。CI 拒绝来源不明 fixture。

## Risks / Trade-offs

- [命令面规模过大] → 以能力矩阵分波次交付，任何发布版本都不得把未实现能力标为支持。
- [Schema v2 与 v1 行为漂移] → 双向 golden fixtures、旧命令别名和兼容期遥测。
- [已有草稿未知字段损坏] → lossless envelope、目标 patch 和源/副本全字段差分。
- [原生接口随剪映版本变化] → Runtime Profile、固定版本 canary 和 fail-closed。
- [任务状态增加持久化复杂度] → ADR-002 已选择 SQLite WAL，并以 revision CAS、并发/恢复/容量测试约束实现。
- [来源污染] → 提交级来源声明、代码审查清单和禁止受限上游 fixture 的 CI 门禁。
- [跨平台功能不对称] → capability 报告包含平台和版本条件，插件按能力而非产品名路由。

## Migration Plan

1. 冻结 v1 行为、固定三来源矩阵和许可证清单。
2. 引入 workspace、领域模型、v1→v2 转换和现有 55 场景回归。
3. 迁移 pyJianYingDraft 完整草稿协议并扩大差分覆盖。
4. 按只读、可逆编辑、创建、批处理、恢复、媒体智能顺序覆盖 86 项能力。
5. 加入任务、审批、配置和 MCP 机器接口。
6. 自主实现已有草稿编辑、ASR、运行时控制和原生导出。
7. 发布带旧命令别名的过渡版本，供插件双读验证但只执行 Rust。
8. 三宿主与真实剪映验收通过后，进入旧命令移除周期。

回滚时保留 v1 parser 和旧二进制；草稿写入通过副本与快照恢复，不回滚用户已在剪映内继续修改的目标草稿。

## Open Questions

- Windows 原生导出的首个支持版本可根据可获得的合法测试环境确定，不改变平台 capability 机制。
