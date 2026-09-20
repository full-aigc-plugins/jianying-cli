# 验收证据等级

机器事实源为 `provenance/EVIDENCE_MODEL.json`，单条证据使用
`schemas/evidence-record-v1.schema.json`。等级只能表达已经观察到的事实，不能按“理论上应该工作”
自动晋级。

| 等级 | 能证明什么 | 不能证明什么 |
|---|---|---|
| `unit` | 单个类型、状态机或门禁满足隔离契约 | 上游 parity、真实草稿可打开 |
| `differential` | 相同 fixture 对固定合法参考具有声明的等价关系 | 真实编辑器兼容 |
| `structural` | 草稿结构、引用和 wire 校验通过 | 编辑器实际接受或播放 |
| `app-open` | 具名平台、产品、版本和文件身份打开草稿 | 重启后仍可打开、播放正确 |
| `cold-reopen` | 编辑器完全关闭后重新启动仍可打开持久草稿 | 时间线实际播放正确 |
| `playback` | 真实编辑器播放并观察到目标画面/音频语义 | 原生导出成功 |
| `native-export` | 显式审批后由具名编辑器产生并哈希验证导出制品 | 其他版本、平台也兼容 |

约束：

- 合成进程和代理渲染最高只能作为 `structural` 证据。
- `app-open` 及以上必须记录平台、产品、精确版本和 runtime identity。
- `native-export` 必须记录审批 ID、草稿摘要、输出 SHA-256 和原生导出观察。
- 任何失败或阻断证据不能被聚合成通过。
- 高等级证据不会反向替代 unit/differential 的回归门禁；各等级解决不同问题。

当前 Runtime/ASR/原生导出任务测试最高为 unit/structural。真实剪映证据只有完成 9.3 canary 后
才能写入 app-open、cold-reopen、playback 或 native-export 记录。
