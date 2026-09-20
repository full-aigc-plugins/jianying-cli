# Persistent job lifecycle

`jianying job run` 从 1.6.0 起在执行前写入持久 journal。默认状态目录是 Job 文件
旁的 `.jianying-jobs/`，数据库文件为 `jobs.sqlite3`；也可用 `--state-root` 或
`JIANYING_STATE_ROOT` 指定状态目录。

```bash
jianying job run job.json --out draft --state-root state --json
jianying job list --state-root state --json
jianying job show <task-id> --state-root state --json
jianying job cancel <task-id> --state-root state --json
jianying job retry <task-id> --state-root state --json
jianying job audit <task-id> --state-root state --json
jianying job maintenance --state-root state --json
```

失败信封保留原始错误类型，同时在 `details.task_id` 提供恢复 ID，并在 `recovery`
给出 retry 命令。首个生产后端选择 SQLite WAL：任务更新使用 `BEGIN IMMEDIATE` 和
revision CAS，避免多个 worker 对同一任务发生静默丢失更新。JSON 文件实现继续保留
用于兼容、对照测试和显式迁移工具，但不再是默认运行路径。选择依据见
`docs/adr/ADR-002-job-store-sqlite.md`。
