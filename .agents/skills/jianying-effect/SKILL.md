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

## 已在真机面板核验的样本

在剪映 **11.7.0.14614**（2026-10-22）的「特效」面板上逐个点开读到的分类：

热门、万圣节、基础、动感、氛围、创意、暗黑、边框、多屏、音效、光、景心、音频、旅行、自然、奇幻、复古、电影、bling、扭曲、综合、分屏、宠物、投影、绘画、黑屏、人物特效

下面这些面板名称在内嵌目录里能命中，因此可以直接引用其真实编号（其余面板项要么不在当前目录快照里，要么已被改名，别凭名字猜编号）：

- `7051836120224502308` — 放大镜
- `6739752823140913675` — 模糊
- `6721949326022545928` — 天使光
- `6740540228194275844` — 爆炸
- `6717433583289504268` — 星光
- `7041474808986472967` — 荧光扫描
- `7456695082293400115` — 金粉飘落（VIP）
- `7447417965676335667` — 放大镜视角（VIP）

未命中的例子：海边光斑、多彩涂鸦、丁达尔光线、碎片重叠、水中倒影。证据：`docs/runbooks/evidence/official-resource/PANEL_NAME_BRIDGE_20261022.json`。

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
