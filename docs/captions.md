# 字幕命令

`jianying captions` 直接操作既有草稿，所有写命令均经过 `MutationPlan` 的快照、
工作副本、引用校验和原子提交；剪映/CapCut 正在运行时会拒绝写入。

## 查询和编辑

```bash
jianying captions list ./draft --json
jianying captions get ./draft <segment-id> --json
jianying captions add ./draft '新增字幕' 4s 1.5s --track 字幕 --json
jianying captions set ./draft <segment-id> '替换后的字幕' --json
jianying captions style ./draft <segment-id> \
  --size 7.5 --color '#12AB34' --bold true --alignment 1 --json
```

文本长度按 UTF-16 码元计算，基础样式范围会随文本变更自动重算；额外样式范围会
被约束到新的文本边界。颜色只接受 `#RRGGBB`，对齐值为 `0/1/2`。

## SRT 与 ASS

```bash
jianying captions import-srt ./draft captions.srt --track 字幕 --offset 500ms --json
jianying captions import-ass ./draft captions.ass --track ASS --json
jianying captions export-srt ./draft --out captions.srt --json
jianying captions export-ass ./draft --out captions.ass --json
```

导出顺序固定为时间、轨道、片段顺序，输出文件通过同目录临时文件替换。

## 翻译

离线映射适合人工、模型或批处理系统先生成翻译，再由 CLI 安全写回：

```json
{
  "by_id": {"<segment-id>": "Translated text"},
  "by_text": {"源文本": "目标文本"}
}
```

```bash
jianying captions translate ./draft translations.json --json
```

也可以使用绝对路径的结构化进程 provider。CLI 不经过 shell，向 stdin 写入一个
`jianying-caption-translation/v1` JSON 文档；provider 必须在 stdout 返回上述
`by_id`/`by_text` 映射：

```bash
jianying captions translate ./draft \
  --provider /absolute/path/to/translator --to zh-CN --json
```

CLI 不隐式联网、不选择供应商，也不会把失败或未知 segment id 的结果部分写入草稿。
