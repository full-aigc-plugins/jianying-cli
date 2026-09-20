# capcut-cli 86 项命令证据门禁

`provenance/PARITY_MATRIX.json` 保存固定提交上的命令语义、Rust 落点与状态；
`provenance/CAPCUT_COMMAND_EVIDENCE.json` 为同样顺序的 86 项逐一绑定仓库内
`test_ref` 和 `evidence_ref`。

状态与声明严格分离：

- `supported` 必须使用 `verified`，且测试不能只引用 gap guard。
- `partial` 必须使用 `gap-recorded`，表示落点已知但尚未满足完整验收。
- `external_dependency` 必须使用 `external-gate`，表示需要真实宿主或外部能力。
- `overall_claim` 在存在 `partial` / `external_dependency` 时只能是 `incomplete`。

CI 执行：

```bash
python3 tools/check_provenance.py --self-test-negative-cases
```

门禁校验固定提交、86 项顺序、状态一致性、引用文件存在性，并执行一个负例：把
`overall_claim` 强制改为 `complete` 后必须失败。因此，“有映射”或“有骨架”不能被
升级成“capcut-cli 全能力完成”。

当前清单中的 gap 仍保留在各自 command 行；实现新的命令差分后，应同步修改 parity
状态、测试链接和证据链接，再运行完整门禁。

`materials` 与 `material` 已由
`tools/capcut_material_discovery_differential.py` 对固定提交 `49f70e3b…` 执行真实进程级
差分，覆盖素材类型计数、按类型摘要、大小写无关 ID 前缀查询和无损详情；结果固化在
`provenance/CAPCUT_MATERIAL_DISCOVERY_DIFFERENTIALS.json`。

`export-timeline` 的 OTIO Timeline.1 文档、文件摘要和字幕 Marker 模式已由
`tools/capcut_interchange_differential.py` 对同一固定提交执行真实进程级完全相等差分，
证据固化在 `provenance/CAPCUT_INTERCHANGE_DIFFERENTIALS.json`。`import-timeline`
的新建草稿、字幕 Marker、占位素材和 dry-run 也在同一固定提交上完成规范化等价差分。
Rust 对不存在的媒体路径采用更严格的完整性策略：草稿素材路径归一为空字符串，原路径留在
`placeholders` 诊断中；该差异已显式批准，不会制造可保存但断链的绝对路径。
