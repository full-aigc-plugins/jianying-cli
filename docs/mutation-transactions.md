# 草稿写事务与恢复

所有原位草稿修改使用 `MutationPlan`，不直接覆盖源目录。当前 `project migrate`
与原位 `project concat` 已接入该路径，后续 timeline、media、captions 和 template
写命令也必须复用它。

## 阶段

1. `plan`：只构造计划，不写磁盘。
2. `snapshot`：在草稿父目录下的 `.jianying-transactions/<id>/snapshot` 保存原始草稿。
3. `work-copy`：仅在隔离副本上修改。
4. `validate`：检查双时间线镜像、元数据路径、素材注册和引用闭包。
5. `atomic-commit`：通过同父目录 `rename` 激活已校验副本；第二次 `rename`
   失败时立即恢复原草稿。

校验失败只会将事务置为 `rejected`，源草稿不变。成功提交后原始 snapshot
仍保留，便于人工检查或恢复。

## 审计与恢复

`audit.json` 记录有序阶段、操作、错误和针对该快照生成的恢复命令：

```bash
jianying store restore-snapshot \
  --snapshot '<transaction>/snapshot' \
  --target '<draft>'
```

恢复本身也是一次新的 `MutationPlan`：它先为当前目标创建安全快照，将待恢复
快照拷入新工作副本，重定位草稿元数据，完整校验后才原子提交。检测到
剪映或 CapCut 运行时恢复会拒绝执行。
