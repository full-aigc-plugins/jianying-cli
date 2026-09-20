# ADR-002：首个生产 Job Store 采用 SQLite WAL

- 状态：Accepted
- 日期：2026-09-19
- 范围：`jianying job run/batch/serve/list/show/cancel/retry/audit/maintenance`

## 背景

外部 task 契约不能依赖进程内状态。候选实现为每任务一个 JSON 文件，以及本地 SQLite。
对比必须覆盖并行写入、执行中断恢复和数据量，不以单次耗时决定正确性。

## 可执行证据

`crates/jianying-jobs/tests/backend_comparison.rs` 固定验证：

1. 8 个线程并行写入 256 条独立记录，两个后端均不得丢记录；测试输出各自耗时，但不设置易波动的性能阈值。
2. 两个 worker 从相同 revision 更新同一任务时，SQLite 必须只接受一个提交，另一个返回 `RevisionConflict`。
3. SQLite 连接在事务提交前中断后，重新打开只能看到已提交记录；JSON 对照实现忽略残留临时文件。

2026-09-19 本机 debug 测量为 JSON 36ms、SQLite 116ms。该数字只描述本次环境，不作为跨平台性能承诺。

## 决策

首个生产后端采用 bundled SQLite，启用 WAL、5 秒 busy timeout、`BEGIN IMMEDIATE` 和
revision compare-and-swap。外部仍传状态目录，内部固定使用 `<state-root>/jobs.sqlite3`，
因此 CLI 参数和 task JSON 契约不变。

JSON `JobStore` 保留为对照和兼容实现，但默认 handler 不再调用它。审批记录暂不纳入本
ADR；审批并发消费将在接入具体写事务前采用同一 SQLite/CAS 原则。

## 取舍与后果

- 接受 bundled SQLite 带来的二进制体积和串行写事务开销。
- 获得事务回滚、崩溃恢复、跨线程/进程协调和明确的丢失更新检测。
- 不把 256 条 debug 测量外推为生产容量；后续发布门禁仍需目标平台 release-profile 压测。
- 现有未发布 JSON journal 不自动混入 SQLite。若形成外部用户数据，必须提供显式、可审计的迁移命令后再移除 JSON 实现。
