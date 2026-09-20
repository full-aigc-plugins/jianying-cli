# 配置 Profile 与宿主只读模式

CLI 使用全局 `--profile <name>` 选择隔离配置。profile 名仅允许 ASCII 字母、数字、`-`
和 `_`，每个 profile 独立存储在 `<config-root>/profiles/<profile>.json`。默认 profile
为 `default`。

配置根目录按以下优先级解析：

1. `JIANYING_CONFIG_ROOT`
2. `$XDG_CONFIG_HOME/jianying-cli`
3. `$HOME/.config/jianying-cli`
4. 当前目录 `.jianying-config`

## 命令

```bash
jianying --profile studio config file --json
jianying --profile studio config schema --json
jianying --profile studio config validate --json
jianying --profile studio config get [render.quality] --json
jianying --profile studio config set render.quality '"high"' --json
jianying --profile studio config patch '{"render":{"threads":4}}' --json
jianying --profile studio config unset render.quality --json
```

`set` 的值与 `patch` 参数都是 JSON。`patch` 对 `values` 使用 RFC 7396 JSON Merge
Patch 语义，其中 `null` 删除字段。写入采用同目录临时文件加 rename；schema、validate、
get 和 file 不创建配置文件。

宿主可使用全局 `--host-read-only` 或 `JIANYING_HOST_READ_ONLY=1` 禁止 set、patch、
unset。只读模式仍允许 file、schema、validate 和 get，拒绝写入时 JSON 错误类型稳定为
`config_read_only`。
