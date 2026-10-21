---
name: jianying-effect
description: 特效（video effects） 的官方素材清单与引用规则。Use when an edit needs a concrete effect id from the JianYing catalog before writing a jianying-job/v2 action.
license: Apache-2.0
metadata:
  generator: tools/gen_resource_skills.py
---

# 特效（video effects）

## 何时使用

要加氛围、粒子、光效、人物特效时使用。

## 怎么查（不要凭记忆编编号）

```bash
jianying media catalog --domain video_scene_effects --search 关键词 --json
```

`--include-vip` 才会返回 VIP 素材；不带它时只返回基础素材。

## 怎么用（Job v2 绑定）

字段：`segment.effect / filterEffectSegment`。每个 action 至少要写清三件事：**用哪个素材（resource_id）**、**放在什么时间（start_us / duration_us）**、**什么效果参数**。

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

- 场景特效覆盖整段画面；人物特效依赖识别到的人体，纯素材片段可能不触发。
- 编号是 `resource_id`（16 位以上数字串），不是 `effect_id`，两者不要混用。
- 表里的每一行都由脚本从 `catalogs/` 复制而来，`--check` 会拦住手工改动。

## 清单

| 目录 | 内容 | 条数 | 全量表 |
| --- | --- | --- | --- |
| `video_scene_effects.json` | 场景特效 | 1097 | [references/video_scene_effects.md](references/video_scene_effects.md) |
| `video_character_effects.json` | 人物特效 | 240 | [references/video_character_effects.md](references/video_character_effects.md) |

合计 **1337** 条。

## 先看这几条（示例，不是全部）

- `6981791065204331044` — 1998
- `6706773500792689165` — 70s
- `6760946510910722564` — 90s画质
- `7130585796020539941` — CCD闪光
- `6878115805498708493` — DV录制框
- `6974600764027048462` — DV界面
- `6899746903735407117` — I Lose You
- `6899746786294895112` — I Love You
