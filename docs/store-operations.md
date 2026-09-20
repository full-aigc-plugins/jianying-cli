# 草稿库操作

`jianying store` 管理草稿目录、根登记表和时间线镜像。写操作在剪映/CapCut 运行时
fail closed，名称必须是单个安全路径组件。

```bash
jianying store directories --json
jianying store list --root ./draft-store --json
jianying store register ./built-draft --root ./draft-store --json
jianying store rename old-name new-name --root ./draft-store --json
```

`register` 是 `publish` 的兼容命令名：验证输入草稿、复制到草稿库、重定位素材并写入
`root_meta_info.json`。重命名保留 draft id，并同步时间线名称、sidecar 路径和根登记项。

## 时间线镜像

`draft_content.json` 是同步命令的显式 canonical。默认只输出计划；`--apply` 仅覆盖发生
漂移的 `draft_info.json` / `template-2.tmp`，每个被写文件先生成 `.bak`。如果镜像时间
晚于 canonical，必须人工检查或显式传 `--force-newer`。

```bash
jianying store sync ./draft --json
jianying store sync ./draft --apply --force-newer --json
```

## 备份与恢复

```bash
jianying store backup ./draft --out ./backups/draft-001 --json
jianying store restore ./backups/draft-001 --target ./draft --json
```

备份目标必须不存在且输出本身通过 bundle 校验。恢复使用 `MutationPlan` 的安全快照、
工作副本、校验和原子提交，并返回恢复审计路径。

## 加密检测边界

```bash
jianying store decrypt ./draft --json
```

该命令名称保持上游兼容，但能力是“检测并解释”，不是绕过加密：可区分明文 JSON、
以 `{` 开头但损坏的 JSON，以及符合剪映 6.0+ 加密特征的非 JSON 二进制。CLI 不包含
反向工程密钥或解密算法；加密草稿必须留在原生 Runtime Adapter 边界内处理。
