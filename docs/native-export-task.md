# 原生导出任务契约

原生导出与 ffmpeg 代理渲染是两个不同的类型和证据等级。当前实现完成授权、持久状态、进度、
中断恢复和结果验证；真实剪映 adapter 与真实宿主 canary 尚未完成，因此 `render.native` 仍不得
宣称 supported。

## 启动门禁

`NativeExportSubmission` 只接受 `ExportKind::Native`，并要求：

- Runtime Profile 声明 `render.native`；
- Runtime Probe 已通过产品、版本、平台、文件身份、权限、草稿根、素材和 capability 检查；
- Probe 中的可执行文件身份与 Profile 完全相同；
- 草稿目录存在，cwd 与输出路径均为绝对路径；
- 草稿全树快照计算完成。

审批命令固定为 `render.native`，精确绑定 profile、可执行 SHA-256、草稿全树 SHA-256、输出、
覆盖策略、cwd、target 和 task ID。审批消费成功并持久化 queued 记录后，adapter 才能启动。

`render native-task` 是 adapter 与持久任务之间唯一的 CLI 回调面：

```text
jianying render native-task show <task-id> --state-root <dir>
jianying render native-task start <task-id> --state-root <dir>
jianying render native-task progress <task-id> <0..99> --state-root <dir>
jianying render native-task verify <task-id> --state-root <dir>
jianying render native-task interrupt <task-id> --reason <text> --state-root <dir>
jianying render native-task fail <task-id> --reason <text> --state-root <dir>
jianying render native-task result <task-id> --state-root <dir>
```

`start` 只能消费 queued 状态，`progress` 只能单调推进 running 状态。`verify` 从 running 进入
verifying，并以磁盘上的目标文件完成非空、覆盖变化和 SHA-256 校验；若进程在文件已生成后中断，
对 verifying 状态重跑 `verify` 不重复启动 adapter。`result` 会重新读取文件并拒绝长度或摘要漂移。

## 状态和恢复

状态机为：

```text
queued -> running -> verifying -> succeeded
                    |             |
                    +-> failed <--+
                    +-> interrupted

failed|interrupted -> queued 仅允许新审批下显式 retry
```

运行进度只能在 `running` 状态按 `0..99` 单调增加。中断记录保留最后进度、终态原因和结构化检查
argv：`jianying render native-task show <task-id>`。调用方随后必须重新生成并消费新审批，再以完全
相同的 `render native` profile、执行器身份、草稿哈希、输出和覆盖策略提交；CLI 检测到 failed 或
interrupted 记录后调用专用原生导出 retry，任何字段漂移都会拒绝，绝不错误路由到普通 `job retry`。

## 结果证明

adapter 报告完成后先进入 `verifying`。只有目标文件存在、非空且 SHA-256 已记录，才能进入
`succeeded`。若覆盖既有输出，新结果必须与启动前哈希不同。后续读取会重新校验长度和 SHA-256，
漂移后不再返回可信结果。

`NativeExportArtifact` 的 kind 在构造时固定为 `native`；`proxy` 请求在 submission 构造阶段直接
拒绝，不能依靠改标签冒充原生导出。

合成测试只证明任务治理契约，不证明真实剪映产生了视频。真实 `app-open`、`cold-reopen`、
`playback` 和 `native-export` 证据必须由 9.3 的合法 macOS 宿主 canary 提供。
