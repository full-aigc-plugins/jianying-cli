---
name: jianying-filter
description: 滤镜（filters） 的官方素材清单与引用规则。Use when an edit needs a concrete filter id from the JianYing catalog before writing a jianying-job/v2 action.
license: Apache-2.0
metadata:
  generator: tools/gen_resource_skills.py
---

# 滤镜（filters）

## 何时使用

要给片段整体调色时使用：复古、胶片、冷调、美食等。

## 怎么查（不要凭记忆编编号）

```bash
jianying media catalog --domain filters --search 关键词 --json
```

`--include-vip` 才会返回 VIP 素材；不带它时只返回基础素材。

## 怎么用（Job v2 绑定）

字段：`filterEffectSegment`。每个 action 至少要写清三件事：**用哪个素材（resource_id）**、**放在什么时间（start_us / duration_us）**、**什么效果参数**。

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

- 滤镜按片段生效，参数在 `params` 里；VIP 滤镜需要专业权益。
- 编号是 `resource_id`（16 位以上数字串），不是 `effect_id`，两者不要混用。
- 执行端**编号与显示名都接受**（2026-10-22 在 11.7 上逐类型实测：特效/滤镜/转场/字体/文本动画/蒙版均落地到草稿）；传编号时草稿里的显示名会保留那串数字，属外观问题不影响渲染。
- 表里的每一行都由脚本从 `catalogs/` 复制而来，`--check` 会拦住手工改动。

## 清单

| 目录 | 内容 | 条数 | 全量表 |
| --- | --- | --- | --- |
| `filters.json` | 滤镜 | 1052 | [references/filters.md](references/filters.md) |

合计 **1052** 条。

## 先看这几条（示例，不是全部）

- `7127828208690433311` — 1980
- `7127679308897832206` — ABG
- `7195816046077496635` — Ditto
- `7127819154018536741` — KE1
- `7127578859217620254` — KV5D
- `7232216810031025468` — Lofi II
- `7127669764905782542` — VHS III
- `7127669338089311495` — 三洋VPC
