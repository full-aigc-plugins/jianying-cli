## Purpose

定义在用户合法安装的剪映环境中进行已有草稿编辑、运行时控制、ASR 和原生导出的自主 Rust 能力，同时保持隔离、授权和失败可恢复。

## ADDED Requirements

### Requirement: 本机编辑器只读发现
系统 SHALL 在不启动应用、不写草稿、不生成受支持 profile 的前提下发现已知剪映/CapCut 安装与
草稿根，并固定 executable 内容身份和可得版本。发现结果必须保持 unverified 且禁止自动路由。

#### Scenario: 草稿存在但应用已缺失
- **WHEN** 已知草稿根存在，但所有已知应用搜索根都没有匹配 executable
- **THEN** `runtime discover` 返回 `drafts_without_editor`、现有草稿根和 `automatic_routing=false`，不得推断原生控制可用

#### Scenario: 发现应用安装
- **WHEN** 已知应用包包含匹配 executable
- **THEN** 返回产品、平台、安装根、版本（若可得）、executable SHA-256/长度及关联草稿根，但 support status 仍为 unverified，下一步要求精确 Runtime Profile canary

#### Scenario: Windows Runtime Profile 尚无真实 canary
- **WHEN** 发布制品声明 `runtime.profile.windows` 缺失、平台不是 Windows、状态不是 `external_dependency`，或 availability 不是 `unsupported`
- **THEN** 发布运行时验证器和独立打包器都必须 fail closed；即使绕过 CI 步骤顺序也不能生成制品，只有真实 Windows 主机完成版本化 Runtime Profile canary 后，才允许通过后续规格变更提升支持状态

### Requirement: 已有草稿隔离编辑
系统 MUST 默认从源草稿创建独立副本并验证源文件未变化，不得原地覆盖用户草稿。

#### Scenario: 编辑已有多轨草稿
- **WHEN** 用户请求修改已有草稿中的文字和音量
- **THEN** 系统在新副本执行修改，并提供源草稿未变化的验证证据

### Requirement: 原生导出显式授权
系统 SHALL 仅在用户明确请求并且运行时、版本、素材与权限检查通过后启动原生导出；代理渲染不得标记为原生导出。

#### Scenario: 未明确要求原生导出
- **WHEN** 用户只要求生成可编辑草稿
- **THEN** 系统不启动剪映导出流程并返回后续人工导出提示

### Requirement: ASR 幂等账本
系统 SHALL 按源内容哈希和执行器身份记录 ASR 请求，复用已完成结果，并阻止不明状态请求被静默重复提交。

#### Scenario: 重复请求相同素材
- **WHEN** 相同素材与执行配置已经存在完成记录
- **THEN** 系统返回已验证结果且不再次调用 ASR 执行器

### Requirement: 运行时探测与安全控制
系统 SHALL 在修改草稿库或调用编辑器前验证产品、版本、进程状态、文件身份和所需 capability；不支持的组合必须 fail closed。

#### Scenario: 编辑器仍在运行
- **WHEN** 写入草稿库时检测到剪映仍在运行
- **THEN** 系统拒绝写入并返回关闭编辑器后的恢复命令

### Requirement: 自主实现来源隔离
系统 MUST NOT 包含或派生自非商业 headless 仓库的源码、测试、蓝图、资源目录或实现常量；能力只能依据本项目自主设计、Apache/MIT 来源和合法黑盒验证实现。

#### Scenario: 新增原生能力提交
- **WHEN** 提交引入已有草稿编辑或原生导出能力
- **THEN** 来源门禁要求对应设计记录、独立测试证据和不含受限来源内容的审查结果
