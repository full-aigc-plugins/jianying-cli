---
name: jianying-text
description: 文本与花字（fonts + text animations） 的官方素材清单与引用规则。Use when an edit needs a concrete text id from the JianYing catalog before writing a jianying-job/v2 action.
license: Apache-2.0
metadata:
  generator: tools/gen_resource_skills.py
---

# 文本与花字（fonts + text animations）

## 何时使用

要写字幕、标题、花字，或给文本加入场/出场/循环动画时使用。

## 怎么查（不要凭记忆编编号）

```bash
jianying media catalog --domain fonts --search 关键词 --json
```

`--include-vip` 才会返回 VIP 素材；不带它时只返回基础素材。

## 怎么用（Job v2 绑定）

字段：`textSegment（含 font、animation）`。每个 action 至少要写清三件事：**用哪个素材（resource_id）**、**放在什么时间（start_us / duration_us）**、**什么效果参数**。

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

- 文本片段必须落在 text 轨道；字体 id 与动画 id 是两套编号，不要混用。
- 编号是 `resource_id`（16 位以上数字串），不是 `effect_id`，两者不要混用。
- 执行端**编号与显示名都接受**（2026-10-22 在 11.7 上逐类型实测：特效/滤镜/转场/字体/文本动画/蒙版均落地到草稿）；传编号时草稿里的显示名会保留那串数字，属外观问题不影响渲染。
- 表里的每一行都由脚本从 `catalogs/` 复制而来，`--check` 会拦住手工改动。

## 清单

| 目录 | 内容 | 条数 | 全量表 |
| --- | --- | --- | --- |
| `fonts.json` | 字体/花字 | 798 | [references/fonts.md](references/fonts.md) |
| `text_animations_in.json` | 文本入场动画 | 145 | [references/text_animations_in.md](references/text_animations_in.md) |
| `text_animations_out.json` | 文本出场动画 | 97 | [references/text_animations_out.md](references/text_animations_out.md) |
| `text_animations_loop.json` | 文本循环动画 | 93 | [references/text_animations_loop.md](references/text_animations_loop.md) |

合计 **1133** 条。

## 先看这几条（示例，不是全部）

- `6841117833447870990` — 3D 만화
- `7246698168547545602` — A1明朝
- `7241838813960475173` — AlexBrush
- `7238874837500826170` — Alice-Regular
- `7238879091334582821` — Alike-Regular
- `7239181917067678266` — Allura-Regular
- `7236679983140180539` — Amble-Regular
- `7042231710112551431` — Amigate
