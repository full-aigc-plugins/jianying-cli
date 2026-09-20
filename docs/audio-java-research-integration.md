# Java 音频研究整合基线

## 来源边界

研究来源固定为 `partme-ai/spring-ai-examples` 提交
`b7740bb945bc08257be29bbdbf510924f27e76b4`。该仓库当前未发现仓库级 LICENSE，因此本项目
只独立重写可观察的协议事实、领域结构与测试场景，不复制 Java 源码。机器可读清单见
`provenance/AUDIO_JAVA_RESEARCH_SOURCES.json`。

这些示例把 Ollama 用于上游文本生成，但音频由独立进程或服务完成。它们不能证明 Ollama
本身具有 TTS 或 ASR endpoint。

## 整合矩阵

| 研究模块 | 真实边界 | 可复用协议事实 | Rust 归属 | 当前状态 |
|---|---|---|---|---|
| ChatTTS | 本机 ChatTTS-ui HTTP wrapper | 表单 `/tts`、生成参数、URL/文件响应 | 受限本地 TTS codec | codec/离线响应测试完成；模型非商用，握手待完成 |
| EdgeTTS | 本机 CLI 调用微软在线语音服务 | 结构化 argv、voice/rate/volume、媒体输出 | 显式联网 TTS Provider | 安全进程 adapter 完成；真实联网 canary 待授权 |
| EmotiVoice | 回环 HTTP 本地运行时 | OpenAI 风格 `/v1/audio/speech` | 具名本地 Runtime Profile | codec/二进制响应测试完成；运行时握手待完成 |
| MARS5-TTS | 本地模型进程 | Java 模块当前误复用 ChatTTS-ui schema | 外部进程 Runtime Profile | 拒绝迁移伪协议；AGPL 门禁 |
| UnifiedTTS | 需要 API Key 的远程聚合服务 | 模型/音色发现、同步合成、音频 URL | 付费云 Provider | 必须接费用账本与下载门禁 |
| Whisper | 本地或兼容服务 ASR | multipart 转录、翻译、时间戳、SRT/VTT | ASR Provider | 不进入 TTS 注册表 |

## 独立重写规则

- ChatTTS codec 可以使用字段级 fixture 重写，但商业 Job 必须被模型许可证门禁拒绝。
- EdgeTTS 复用现有无 shell `local-process` 执行模型；所有参数保持结构化 argv，输出文本不得写入
  命令日志。因其依赖外网，不得归为离线本地模型。
- EmotiVoice 必须先探测回环 endpoint，再启用 OpenAI Audio 兼容 codec；未握手时保持 partial。
- MARS5-TTS 不采用 Java 示例中的 `/tts` schema，只允许独立实现的本地进程 adapter。AGPL
  运行时保持外部进程边界，分发或网络服务使用前执行 copyleft 合规审查。
- UnifiedTTS 的合成和音频下载都属于外部付费操作。提交前需要审批和幂等账本；返回的
  `audio_url` 必须经过 HTTPS、Host allowlist、Content-Type、长度上限和内容哈希验证。
- Whisper 迁入 ASR Provider：支持 `json/text/srt/verbose_json/vtt`，只有 `verbose_json` 可以请求
  word/segment 时间戳。ASR 账本仍使用 queued/running/succeeded/failed/ambiguous 状态。

## 支持声明门禁

来源清单、协议 codec 单元测试或编译成功都不足以报告 `supported`。每个 provider 还必须具备：

1. 固定安装制品版本、路径与哈希。
2. capability probe 和启动/endpoint 握手。
3. 离线 fixture 合约测试。
4. 对本机运行时执行真实音频 canary；对远程服务执行单独授权的费用 canary。
5. 输出音频格式、时长和内容哈希验证。
