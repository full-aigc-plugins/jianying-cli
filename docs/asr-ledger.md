# ASR Provider 与幂等账本

ASR 使用与 TTS 分离的 Provider 和账本契约。当前已接入官方 whisper.cpp `whisper-cli` 的
本地结构化进程 adapter；不打包、不下载 whisper.cpp 二进制或模型权重，也不把云厂商宣称为
已经接入。

## Provider 合约

`AsrProviderCapability` 固定：

- Provider ID；
- 含版本或制品哈希的 executor identity；
- 本地或付费属性；
- 支持的 `json`、`text`、`srt`、`verbose_json`、`vtt` 格式；
- 支持的 segment/word 时间戳粒度。

`AsrRequest` 只保存源内容 SHA-256，不保存音频正文。时间戳只允许用于 `verbose_json`，Provider
未声明的格式或粒度会 fail closed。`AsrArtifact` 绑定 Provider、源内容哈希、格式、路径和非空
字节数。

## 幂等键与状态机

幂等键由以下字段使用长度前缀编码后计算 SHA-256：

- Provider ID；
- executor identity；
- 源内容 SHA-256；
- 完整规范化请求 JSON。

状态机为 `queued → running → succeeded|failed|ambiguous`。`ambiguous` 只能在外部确认没有接受
请求后转为 `failed`；`failed` 只能经显式 retry 回到 `queued`。`attempts` 仅在真正进入
`running` 时递增。

首次提交使用跨进程可见的 `create_new` claim。同一幂等键只能有一个调用方得到 `Submit`；无法
判断前一个进程是否已经外部提交时返回 `ambiguous`，不会自动重提。成功记录仅在产物仍存在且
SHA-256 匹配时返回 `Reuse`。

## 费用与隐私边界

本地请求必须声明零预算并走 `prepare_local`。付费请求必须声明正预算并走 `prepare_paid`，审批
精确绑定 `asr.cloud.submit`、Provider、executor、源哈希、请求哈希、预算、cwd、target 和 task。
付费失败重试必须使用新审批。

账本不保存音频正文、源路径或秘密值，只保存哈希、执行器身份、目标、状态、审批 ID 和经过验证
的产物信息。

## whisper.cpp 本地执行

```bash
jianying --json media transcribe input.wav \
  --out transcript.srt \
  --executable /absolute/path/to/whisper-cli \
  --model /absolute/path/to/ggml-model.bin \
  --model-id ggml-model \
  --format srt \
  --language zh \
  --task-id transcribe-001 \
  --state-root .jianying-asr \
  --plan
```

`--plan` 只校验并输出 executor identity、源哈希和幂等键，不启动进程或创建账本。去掉
`--plan` 后才执行；失败记录必须显式增加 `--retry`。adapter 使用 `Command` 的结构化 argv，
不会经过 shell；输出先写唯一 partial prefix，验证非空后原子 rename 到 `--out`。可执行文件
与模型内容哈希共同组成 executor identity，因此任一制品变化都会产生新幂等键。

格式支持 `json|verbose-json|text|srt|vtt`；`--segment-timestamps` 仅允许配合
`verbose-json`。来源和“不捆绑二进制/权重”边界记录在
`provenance/ASR_PROTOCOL_SOURCES.json`。

## 证据

- `jianying-media/tests/asr_provider_contract.rs`：格式、时间戳、源哈希和产物契约。
- `jianying-media/tests/whisper_cpp_asr_provider.rs`：官方 CLI 参数映射、无 shell、原子落盘和
  fail-closed 校验。
- `jianying-jobs/tests/asr_ledger.rs`：完成结果复用、付费审批、ambiguous 门禁和并发单提交。
- `tests/asr_cli_contract.rs`：plan 零执行、成功复用、失败显式 retry 和 attempts 计数。

真实模型/音频 canary、local HTTP/云端 adapter 与字幕写回仍属于后续集成和宿主验收。
