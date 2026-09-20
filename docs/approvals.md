# 精确绑定审批

`jianying approvals` 提供本地、带有效期且一次性消费的写操作审批记录。审批绑定以下全部字段：

- `command`
- 有序 `arguments` 及其 SHA-256 摘要
- 绝对 `cwd`
- 绝对 `target`
- `task_id`
- `issued_at` 与 `expires_at`

任一绑定字段变化、记录过期或重复消费都会返回 `approval_denied`。审批不会扩大 capability；调用方仍须在执行前检查 capability manifest。

```bash
jianying approvals grant \
  --command project.publish \
  --arg=--force --arg=false \
  --cwd /workspace --target /workspace/draft \
  --task-id jy-task-1 --ttl-seconds 300 --json

jianying approvals check <approval-id> \
  --command project.publish \
  --arg=--force --arg=false \
  --cwd /workspace --target /workspace/draft \
  --task-id jy-task-1 --json
```

默认审批目录为 `.jianying-approvals`，可以用 `--state-root` 或 `JIANYING_APPROVAL_ROOT` 覆盖。当前 JSON 文件后端使用临时文件加 rename 落盘；并发消费和跨进程锁仍属于任务 7.4 的生产后端决策范围。
