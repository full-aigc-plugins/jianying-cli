# 模板库与预设

模板库是一个显式指定的本地目录。`save` 会生成独立草稿副本并写入
`.jianying-template.json`；`list` 只返回带受支持 manifest 且通过草稿完整性校验的目录。

```bash
jianying template save ./draft social-card \
  --root ./templates --description '竖版社交字幕' --json
jianying template list --root ./templates --json
jianying template apply ./templates/social-card episode-01 \
  --root ./drafts --json
```

应用模板会重建草稿 ID、名称、时间和素材路径，输出草稿不保留模板 manifest，因此不会
被模板库误识别。名称只能是单个安全路径组件，已存在目标不会覆盖。

文字预设是 `jianying-template-preset/v1` JSON，保存首个文字素材的基础样式；应用时
将样式事务化写入全部文字素材，但保留每条文本、额外样式区间和未知字段：

```bash
jianying template make-preset ./styled large-blue --root ./presets --json
jianying template apply-preset ./draft ./presets/large-blue.json --json
```

既有命令继续保留：

```bash
jianying template duplicate ./draft copy --root ./drafts --json
jianying template import-track ./target ./source 字幕 --json
```
