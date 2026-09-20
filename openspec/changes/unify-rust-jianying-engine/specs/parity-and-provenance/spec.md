## Purpose

建立可审计的来源、能力、差分测试和真实运行证据体系，确保“支持”声明同时对应固定版本行为、许可证义务和可重复验证结果。

## ADDED Requirements

### Requirement: 固定版本 parity 矩阵
系统 SHALL 将每项功能、命令和数据结构绑定到固定上游提交、Rust 对应能力、测试和证据状态。

#### Scenario: 上游版本更新
- **WHEN** 任一上游固定提交发生变化
- **THEN** 受影响矩阵项失效并要求重新审查，不得沿用旧版本通过结论

### Requirement: 差分测试
系统 SHALL 对可运行参考实现使用相同输入进行语义差分，并分别记录完全相等、规范化等价和已批准差异。

#### Scenario: Wire shape 漂移
- **WHEN** Rust 输出与参考草稿在规范化后仍存在未批准差异
- **THEN** 差分测试失败并保存最小字段路径差异

### Requirement: 许可证与来源门禁
系统 SHALL 保存 Apache-2.0 和 MIT 来源的许可证、版权、修改及版本证据，并阻止未授权来源进入源码、测试、资源或发布制品。

#### Scenario: 发现非商业来源片段
- **WHEN** 来源审查发现文件或 fixture 来自非商业上游
- **THEN** 构建或发布门禁失败并要求移除或提供书面授权

### Requirement: 证据等级
系统 SHALL 区分单元测试、差分测试、草稿结构验证、剪映冷重开、原生播放和原生导出，不得用较低等级证据宣称较高等级能力。

#### Scenario: 仅结构验证通过
- **WHEN** 草稿 JSON 验证通过但未执行剪映冷重开
- **THEN** 状态只能标记为结构通过，不能标记为真实宿主验收完成

### Requirement: 不可变发布可恢复验证
发布流水线 SHALL 在 GitHub release attestation 最终一致时安全重试；已经公开的 Release 只有在 tag、稳定状态、immutable 状态和逐资产 SHA-256 与当前构建完全一致时才能继续验证，且不得覆盖资产。尚未公开的 Draft 只有在 tag 正确、非 prerelease、尚未 immutable，且已有资产是当前构建资产集合的逐字节相同子集时，才可只上传缺失资产并继续发布。

#### Scenario: Draft 上传中断后安全续传
- **WHEN** 首次发布在 Draft 已创建且仅上传部分预期资产后中断
- **THEN** 重跑必须按名称、uploaded 状态、字节长度和 GitHub SHA-256 验证已有资产，只上传缺失资产，并在完整集合再次验证通过后公开；不得 clobber 已有资产

#### Scenario: Draft 含污染或漂移资产
- **WHEN** 同 tag Draft 含有额外资产，或同名资产的状态、长度、SHA-256 与本次确定性构建不同
- **THEN** 发布必须在公开 Draft 前 fail closed，不得覆盖、删除或把污染集合锁定为不可变 Release

#### Scenario: 同 tag 发布重跑并发
- **WHEN** 同一 tag 的首次运行和重跑同时进入 GitHub Actions 队列，或 Draft 在初次规划后发生变化
- **THEN** workflow 必须按 workflow/ref 保证同一时刻只有一个运行进入发布区且不得取消已运行中的发布；GitHub 替换尚未开始的旧 pending 重跑不构成已运行发布取消。任何实际获准运行的非既有 immutable 路径都必须在公开 Draft 前重新读取远端元数据并验证完整资产集合

#### Scenario: Draft 阶段远端 tag 被移动
- **WHEN** Release 尚为可修改 Draft，远端 lightweight tag 或 annotated tag 的 peeled commit 不再等于本次发布 checkout
- **THEN** 初次规划和公开前最终规划均必须 fail closed；不得把正确资产锁定到不同源码提交

#### Scenario: Draft 公开前缺少 tag 防篡改规则
- **WHEN** 正典仓库不存在覆盖 `refs/tags/v*` 的 active tag ruleset，或该规则允许 bypass、删除、非快进更新或排除目标 tag
- **THEN** 发布预检必须在创建或公开 Draft 前 fail closed；仅重复读取 tag 不能替代远端防篡改规则

#### Scenario: 公开后证明暂未可见
- **WHEN** Release 已公开并锁定，但 `verify-asset` 暂时报告没有 attestation
- **THEN** 流水线轮询完整 Release attestation 和逐资产证明；完整证明必须将 package SHA-1 绑定远端 tag 对象、将全部资产 SHA-256 绑定本次精确资产集合，同时由 publication plan 将 annotated tag 的 peeled commit 绑定发布 checkout；重跑时下载并逐文件验证既有资产后继续，不创建、上传或覆盖同名资产

#### Scenario: 既有资产与本次构建不同
- **WHEN** 同 tag Release 的资产集合或任一 SHA-256 与当前三平台构建不同
- **THEN** 发布立即失败并保留既有不可变 Release，不得通过 clobber 或替换资产恢复

#### Scenario: 相同提交重跑发布证明
- **WHEN** attestation 暂未可见后以相同提交、lock 和固定工具链重跑发布 workflow
- **THEN** SBOM、tar.gz/zip、release-entry 和 checksum sidecar 必须使用 `SOURCE_DATE_EPOCH` 或提交时间及规范化路径、所有者、权限与文件顺序生成逐字节一致的资产，使流水线能够只恢复证明验证而不覆盖 immutable Release

#### Scenario: Runner 镜像工具版本变化
- **WHEN** GitHub runner 更新默认 Python、Node 或 Rust 组件
- **THEN** 来源差分和三平台发布仍使用 workflow 显式固定的 Python 3.12、Node 24、项目 Rust toolchain 与 immutable setup action，不得依赖默认镜像版本

#### Scenario: Release index 指向非正典仓库或重命名制品
- **WHEN** 三平台 entry 使用非 `full-aigc-plugins/jianying-cli` 仓库、archive 名称不精确匹配 `jianying-cli-<version>-<platform>.tar.gz|zip`，或相邻 `.sha256` sidecar 缺失/内容不匹配
- **THEN** index 聚合器在生成下载 URL 前 fail closed；不得生成一个只能由下游插件再次拒绝或缺少公开 checksum 的发布索引

#### Scenario: 发布汇总下载到非平台 artifact
- **WHEN** 同一 workflow 还产生远端预检、诊断或其他非制品 artifact
- **THEN** publish job 必须按 macOS arm64、macOS x64、Windows x64 三个精确 artifact 名称分别下载；不得用宽泛模式把非平台证据混入 release index 输入目录

#### Scenario: SBOM 截断或依赖身份漂移
- **WHEN** 平台 SBOM 遗漏或额外包含任一 `Cargo.lock` 包，依赖名称/版本/来源不同，registry checksum 缺失或漂移，SPDXID 重复，或任一包没有已声明许可证
- **THEN** 打包器必须在生成 archive 和 release-entry 前 fail closed；只声明根包的占位 SPDX 文件不得作为发布 SBOM；相同 lock 与 `SOURCE_DATE_EPOCH` 必须生成逐字节相同的 SPDX 文件

#### Scenario: Tag 构建缺少 Immutable Releases 设置
- **WHEN** tag workflow 启动，而正典 CLI 仓库尚未启用 GitHub Immutable Releases
- **THEN** 来源门禁必须在完整 workspace 测试与三平台 runner 前失败，并无论检查成功或失败都上传结构化预检 JSON；同一检查在手工 `workflow_dispatch` 中仅记录证据，不得阻止 `committed_unreleased` 候选构建
