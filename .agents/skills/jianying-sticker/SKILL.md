---
name: jianying-sticker
description: 贴纸（stickers） 的官方素材清单与引用规则。Use when an edit needs a concrete sticker id from the JianYing catalog before writing a jianying-job/v2 action.
license: Apache-2.0
metadata:
  generator: tools/gen_resource_skills.py
---

# 贴纸（stickers）

## 何时使用

要加表情、箭头、边框、动效贴纸时使用。

## 怎么查（不要凭记忆编编号）

```bash
jianying media catalog --domain <domain> --search 关键词 --json
```

`--include-vip` 才会返回 VIP 素材；不带它时只返回基础素材。

## 怎么用（Job v2 绑定）

字段：`stickerSegment（`resource_id` 必填）`。每个 action 至少要写清三件事：**用哪个素材（resource_id）**、**放在什么时间（start_us / duration_us）**、**什么效果参数**。

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

- 贴纸没有内嵌目录，而且**当前没有受支持的登记路径**（2026-10-22 实测）：`media harvest-enums add` 只接受 video_effects / filters / transitions / masks / audio_effects，传 sticker 会被拒绝；`sync` 又会跳过编辑器建的可加密草稿。因此贴纸编号目前只能由调用方直接给出，执行端只做形状校验（非空、≤64 字符、仅字母数字与 `-_.:`）。
- 编号是 `resource_id`（16 位以上数字串），不是 `effect_id`，两者不要混用。
- 表里的每一行都由脚本从 `catalogs/` 复制而来，`--check` 会拦住手工改动。

## 清单

该类型没有内嵌目录，见下面的采集方式。
