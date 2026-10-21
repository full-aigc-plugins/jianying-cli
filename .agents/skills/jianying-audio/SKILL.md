---
name: jianying-audio
description: 音频（音色 + 场景音效 + 音乐） 的官方素材清单与引用规则。Use when an edit needs a concrete audio id from the JianYing catalog before writing a jianying-job/v2 action.
license: Apache-2.0
metadata:
  generator: tools/gen_resource_skills.py
---

# 音频（音色 + 场景音效 + 音乐）

## 何时使用

要配音、变声、加场景音效或音乐时使用。

## 怎么查（不要凭记忆编编号）

```bash
jianying media catalog --domain tone_effects --search 关键词 --json
```

`--include-vip` 才会返回 VIP 素材；不带它时只返回基础素材。

## 怎么用（Job v2 绑定）

字段：`audioSegment / segment.tone_effect`。每个 action 至少要写清三件事：**用哪个素材（resource_id）**、**放在什么时间（start_us / duration_us）**、**什么效果参数**。

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

- 音色用于改变人声；场景音效是独立素材；在线音乐不在内嵌目录里，见 `media catalog` 之外的说明。
- 编号是 `resource_id`（16 位以上数字串），不是 `effect_id`，两者不要混用。
- 执行端**编号与显示名都接受**（2026-10-22 在 11.7 上逐类型实测：特效/滤镜/转场/字体/文本动画/蒙版均落地到草稿）；传编号时草稿里的显示名会保留那串数字，属外观问题不影响渲染。
- 表里的每一行都由脚本从 `catalogs/` 复制而来，`--check` 会拦住手工改动。

## 清单

| 目录 | 内容 | 条数 | 全量表 |
| --- | --- | --- | --- |
| `tone_effects.json` | 音色 | 57 | [references/tone_effects.md](references/tone_effects.md) |
| `audio_scene_effects.json` | 场景音效 | 85 | [references/audio_scene_effects.md](references/audio_scene_effects.md) |
| `speech_to_songs.json` | 歌声转换 | 6 | [references/speech_to_songs.md](references/speech_to_songs.md) |

合计 **148** 条。

## 先看这几条（示例，不是全部）

- `7255565276819755576` — 台湾小哥
- `7310059412062736946` — 圣诞精灵
- `7310059178133819930` — 圣诞老人
- `7020344898033291790` — 大叔
- `7020345715901600270` — 女生
- `7328088579811316263` — 广告男声
- `7020344978794615327` — 怪物
- `7018011705414259213` — 机器人
