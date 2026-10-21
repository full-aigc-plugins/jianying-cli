---
name: jianying-caption
description: 字幕样式（caption styles） 的官方素材清单与引用规则。Use when an edit needs a concrete caption id from the JianYing catalog before writing a jianying-job/v2 action.
license: Apache-2.0
metadata:
  generator: tools/gen_resource_skills.py
---

# 字幕样式（caption styles）

## 何时使用

要批量生成字幕并套用样式时使用。

## 怎么查（不要凭记忆编编号）

```bash
jianying media catalog --domain <domain> --search 关键词 --json
```

`--include-vip` 才会返回 VIP 素材；不带它时只返回基础素材。

## 怎么用（Job v2 绑定）

字段：`textSegment + 识别/导入的 SRT`。每个 action 至少要写清三件事：**用哪个素材（resource_id）**、**放在什么时间（start_us / duration_us）**、**什么效果参数**。

```json
{
  "schema": "jianying-job/v2",
  "project": {"name": "demo"},
  "operations": [
    {
      "op": "add_segment",
      "track": "video-1",
      "segment": {"material_id": "m1", "start_us": 0, "duration_us": 2000000}
    }
  ]
}
```

> 具体段类型（transition / filterEffect / sticker / text / audio）的完整字段以 `schemas/jianying-job-v2.schema.json` 为准；本技能只负责“选哪个素材”。

## 注意

- 字幕样式不单独成目录：样式由字体 + 文本动画 + 位置组合而成。字幕文本先用 `jianying media transcribe` 得到 SRT，再逐个写 textSegment。
- 编号是 `resource_id`（16 位以上数字串），不是 `effect_id`，两者不要混用。
- 表里的每一行都由脚本从 `catalogs/` 复制而来，`--check` 会拦住手工改动。

## 清单

该类型没有内嵌目录，见下面的采集方式。
