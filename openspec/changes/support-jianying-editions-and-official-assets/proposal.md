## Why

当前 CLI 只能验证剪映安装版本、二进制身份和本地能力，无法区分基础版、专业版账号权益，也无法证明官方素材是否已合法下载并可用于目标交付。需要将运行时版本、账号权益和单项素材许可拆开建模，才能安全启用专业版内容而不把登录状态或 GUI 可见性误报为授权。

## What Changes

- 新增基础版、专业版和未知版型的显式运行时权益模型，并以可过期证据决定能力可用性。
- 新增官方资源收据，覆盖视频/图片、音乐、文字模板、贴纸、特效、转场和字幕样式，绑定资源 ID、来源、单项权益标记、内容身份、授权条款、用途限制和取得时间。
- 新增机器可读的权益/素材校验命令；专业能力在缺少有效专业版证据时 fail closed。
- 允许插件 GUI Adapter 搜索和下载剪映官方素材，但草稿编译与门禁仍由 Rust CLI 执行。
- 保持本地素材和现有 Runtime Profile 向后兼容；不得从应用名称“剪映专业版”推断账号已开通专业权益。

## Capabilities

### New Capabilities

- `jianying-edition-entitlements`: 定义剪映版型、账号权益证据、多类官方资源收据、能力解析和结构化失败行为。

### Modified Capabilities


## Impact

- `jianying-runtime` 新增权益与素材许可领域对象和验证器。
- CLI `runtime`/`media` 命令面新增只读检查和收据登记能力。
- capability manifest、JSON Schema、测试和运行时文档同步更新。
- `jianying-edit-plugin` 后续通过独立 OpenSpec 变更接入 GUI 搜索/下载和 Harness 路由，不在本变更中把 GUI 自动化耦合进 Rust 核心。
