## Why

`jianying-cli` 当前只覆盖 pyJianYingDraft 的主要草稿构建能力和少量 capcut-cli 操作，尚不能作为剪映自动化的唯一 Rust 运行时。项目需要在保留现有 `jianying-cli-plan/v1` 兼容性的前提下，形成统一领域模型、完整命令能力、可恢复任务执行、已有草稿编辑、ASR 与原生导出边界，从而让插件不再依赖 Python 引擎或外部 headless fork。

## What Changes

- 建立固定上游提交的功能级、命令级、数据结构级 parity 清单：pyJianYingDraft `c3318066`、capcut-cli `49f70e3b`，以及仅作为能力目标而不复制实现的 headless 能力集合。
- 将现有“单个 `Segment` 大量可选字段”的计划模型演进为强类型项目、时间线、轨道、片段、素材、资源、编辑操作、快照与审计领域模型。
- 新增版本化作业协议，并继续接收 `jianying-cli-plan/v1`，通过兼容转换进入统一模型。
- 覆盖 capcut-cli 当前 86 个命令对应的可观察能力，但按剪映领域重新分组，不要求复制原命令名称。
- 自主实现已有草稿隔离编辑、原生导出、ASR 账本和本机编辑器控制能力；不得复制非商业上游的源码、测试、蓝图、资源或固定实现细节。
- 引入 OpenClaw 风格的领域命令树、统一 `--json` 契约、`doctor/status/tasks/audit` 运维面、配置 profile、审批门禁和可恢复任务。
- 为每项迁移建立原实现与 Rust 的差分测试、fixture 来源记录和逐能力验收证据。
- **BREAKING**：最终推荐命令树将替代当前扁平命令作为主界面；旧命令在明确的兼容期内保留别名，不静默改变语义。

## Capabilities

### New Capabilities

- `unified-project-model`: 强类型 Rust 领域模型、版本化 Plan/Job Schema、旧计划兼容转换和未知字段保留策略。
- `cli-capability-surface`: 覆盖 capcut-cli 全部命令能力的领域命令树、机器输出、配置、批处理、任务和诊断契约。
- `native-runtime-automation`: 已有草稿隔离编辑、剪映运行时探测与控制、原生导出、ASR、资源/权限边界和可恢复执行。
- `parity-and-provenance`: 三来源能力矩阵、许可证/来源门禁、差分测试、fixture 治理和验收证据等级。

### Modified Capabilities

无。仓库此前没有 OpenSpec 主规格。

## Impact

- 主要影响 `src/`、`tests/`、`tools/`、`docs/`、CI、Cargo workspace 结构和发布制品。
- 下游 `jianying-edit-plugin` 将只调用本 CLI；`jianying-skills` 将只描述统一 Rust 契约。
- pyJianYingDraft 与 capcut-cli 的许可证和版权声明必须进入发布物；非商业 headless 项目不作为代码来源。
- 原生导出和编辑器控制依赖用户本机合法安装的剪映，不能分发官方库、账号数据或受限资源。
