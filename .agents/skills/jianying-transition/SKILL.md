---
name: jianying-transition
description: 转场（transitions） 的官方素材清单与引用规则。Use when an edit needs a concrete transition id from the JianYing catalog before writing a jianying-job/v2 action.
license: Apache-2.0
metadata:
  generator: tools/gen_resource_skills.py
---

# 转场（transitions）

## 何时使用

两个片段之间要过渡时使用：叠化、闪白、推移、故障等。

## 怎么查（不要凭记忆编编号）

```bash
jianying media catalog --domain transitions --search 关键词 --json
```

`--include-vip` 才会返回 VIP 素材；不带它时只返回基础素材。

## 怎么用（Job v2 绑定）

字段：`transitionSegment / segment.transition`。每个 action 至少要写清三件事：**用哪个素材（resource_id）**、**放在什么时间（start_us / duration_us）**、**什么效果参数**。

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

- `is_overlap=true` 的转场会占用前后片段的重叠时长；`duration_us` 是建议时长。
- 编号是 `resource_id`（16 位以上数字串），不是 `effect_id`，两者不要混用。
- 表里的每一行都由脚本从 `catalogs/` 复制而来，`--check` 会拦住手工改动。

## 清单

| 目录 | 内容 | 条数 | 全量表 |
| --- | --- | --- | --- |
| `transitions.json` | 转场 | 453 | [references/transitions.md](references/transitions.md) |

合计 **453** 条。

## 先看这几条（示例，不是全部）

- `7049979667406656014` — 3D空间
- `7397337005387944457` — 上下翻页
- `6724846395116753416` — 上移
- `6724849276100284942` — 下移
- `6858191434294497805` — 中心旋转
- `6955722927161479694` — 云朵
- `6748313807031898627` — 倒影
- `6919369228701143559` — 冰雪结晶
