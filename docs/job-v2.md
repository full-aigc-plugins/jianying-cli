# `jianying-job/v2`

`jianying-job/v2` 是 Rust CLI、插件 Runtime Adapter 与 Codex/ZCode/Kimi
之间的稳定作业信封。JSON Schema 位于
[`schemas/jianying-job-v2.schema.json`](../schemas/jianying-job-v2.schema.json)，
可执行语义校验位于 `jianying-schema` crate。

## 操作

| operation | 项目要求 | 额外要求 |
|---|---|---|
| `create` | `project.type=new` | 内含统一 `DraftProject` |
| `edit` | `project.type=existing` | `source` 与 `output` 必须不同，`operations` 非空 |
| `inspect` / `verify` / `publish` | `project.type=existing` | 只接收 `source` |
| `export` | `project.type=existing` | 必须包含 `export` |
| `batch` | 无单一项目 | `jobs` 非空且不得嵌套 batch |

最小 create 示例见
[`examples/job-v2-create.json`](../examples/job-v2-create.json)。Rust 调用者应通过
`JobV2` 构造器或 `JobV2::parse`，不要直接反序列化后跳过语义校验。

CLI 执行入口：

```bash
jianying job run job.json --out ./new-draft --json
```

`--json` 成功时只向 stdout 写入一个 `{"ok":true,"data":...}` 文档；失败时
写入一个 `{"ok":false,"error":...}` 文档并返回非零退出码。create 作业通过
`--out` 指定目标草稿目录。

edit 作业从 Job 文件所在目录解析相对 `source`/`output`，不使用 `--out`。当前已用
根 CLI 黑盒测试验证的强类型操作为：

- `replace_text`：替换文字片段正文，并保留既有样式和未知字段；
- `move_segment`：把目标片段设置到明确的微秒起点和时长；
- `remove_segment`：删除片段但保留被清空的轨道；
- `add_material`：声明后续片段使用的本地视频、音频或图片；
- `add_segment`：通过统一领域模型编译完整片段语义，并复制素材引用闭包；
- `add_track` / `reorder_track` / `remove_track`：使用稳定轨道 ID 新增、重排或删除轨道。

执行器先把源草稿完整复制到随机临时目录，逐项运行事务化编辑，再将素材路径和
草稿元数据身份统一改写到最终 `output`，完成完整 bundle 校验和源目录逐文件快照
比对后才通过同目录 rename 提交。任何操作失败都会移除本轮生成的临时副本，不会
创建最终输出。新增素材必须先于引用它的片段声明，并且必须在同一 Job 中被消费；
未声明引用、重复声明、未消费声明、不相容轨道类型和重叠片段均以 `invalid_job`
失败并回滚。字体和只含编辑器资源 ID 的素材仍返回 `job.edit.add_material` 的
`incompatible_capability`，不能冒充已经复制了本地引用闭包。

## 版本兼容规则

- `schema` 必须精确为 `jianying-job/v2`；未知主版本 fail closed。
- `jianying-cli-plan/v1` 保留为兼容输入，通过纯转换器生成 v2 create 作业；完整规范化 v1 载荷保存在 `compatibility` 中，不删除 v1 parser。
- `capcut-cli-compile/v1` 是声明式 compile 兼容输入；create 作业可省略重复的 `project`，`job run --out` 直接复用 `project compile` handler。未知 compatibility schema fail closed，并在错误详情中列出两个受支持版本。
- 新增可选字段属于向后兼容；删除字段、改变字段语义或新增必填字段需要新主版本。
- `provenance/CAPABILITIES.json` 是机器可读能力事实源。`partial` 仅说明契约或部分实现存在，插件不得据此自动路由。

## 当前边界

根 CLI 已支持 `job run` 的 create、上述八类 isolated edit、inspect、verify、publish、
proxy export 与非嵌套 batch 分派。尚未实现的 native export 和 archive export 会返回
`incompatible_capability`，不会静默降级。插件可依赖 v2 解析和 `job.run` 机器契约，
但只能按 capability manifest 路由已标记为 `supported` 的细分能力。
