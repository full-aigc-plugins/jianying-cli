# v1 → Job v2 迁移、兼容期与回滚

## 当前事实

- `jianying-job/v2` 是 CLI、MCP 和未来插件 Runtime Adapter 的统一作业协议。
- `jianying-cli-plan/v1` parser 仍在 `src/plan.rs`，并通过 `src/domain_compat.rs`
  转换为 v2 create job；本变更禁止删除它。
- 55 个冻结 v1 fixture 必须继续通过 v1 → v2 → 草稿语义等价门禁。
- 插件尚未切换到 Rust Runtime Adapter，因此当前不存在可回滚的插件切换或已发布制品。

## 旧命令兼容期

旧顶层命令继续调用相同 handler，并只额外输出 stderr 弃用提示：

| 旧命令 | 新命令 |
|---|---|
| `jianying build` | `jianying project build` |
| `jianying verify` | `jianying project verify` |
| `jianying inspect` | `jianying project inspect` |
| `jianying probe` | `jianying media probe` |
| `jianying catalog` | `jianying media catalog` |
| `jianying publish` | `jianying store publish` |
| `jianying render <draft>` | `jianying render proxy <draft>` |

兼容别名不得在 1.x 内删除。未来只有在一个新的 major 版本中，且命令目录、插件 lock、
迁移说明和回滚制品都已发布并经过三宿主验收后，才可以提出删除；v1 输入 parser 不随
这些 CLI 别名一起删除。

## 调用方迁移

1. 先把脚本从顶层别名迁移到领域命令，不改变输入或输出解析。
2. 读取 `jianying commands --json`，根据 `deprecated` / `replacement` 校验调用路径。
3. 新编排统一生成 `jianying-job/v2`；旧 plan 继续原样提交，由兼容转换器处理。
4. 对未知 schema、破坏性版本或不可用 capability 按结构化错误停止，禁止静默降级。
5. 插件切换时只调用锁定 Rust CLI 制品；在 artifact lock 为空时不得进行切换。

## 草稿回滚演练

本地结构演练覆盖了一个真实写事务：创建草稿 → 执行已知 5.9/9.6 wire migration →
确认快照和审计恢复命令 → `store restore-snapshot` → 再次 bundle verify。复现命令：

```bash
cargo test --test agent_contract \
  project_init_quickstart_migrate_and_concat_are_black_box_operational \
  -- --exact --nocapture
```

v1 parser 保留门禁：

```bash
cargo test --test job_v2_contract \
  all_frozen_v1_fixtures_survive_the_v2_compatibility_envelope \
  -- --exact --nocapture
```

机器证据位于 `provenance/MIGRATION_ROLLBACK_EVIDENCE.json`。

## 生产回滚顺序

插件真正切换后，回滚必须分层进行：

1. 停止新 Job 入队，保留 task id、approval 和审计数据库。
2. 将插件 lock 回退到上一份已验收的 Rust CLI checksum；不能用工作区 Cargo 构建替代制品。
3. 对尚未提交的任务直接取消；对已写草稿使用对应事务的 immutable snapshot 恢复。
4. 若用户已在剪映中继续编辑目标草稿，禁止自动覆盖；复制现状和快照后人工选择合并方向。
5. 重新执行 schema 握手、fresh-install 和三宿主同 Job 验收，再恢复入队。

当前 artifact lock 仍为空，所以第 2、5 步只能作为发布后的演练门禁，不能声明已经完成。
