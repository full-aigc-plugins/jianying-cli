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

- 贴纸没有内嵌目录，但**可以从应用缓存里逆向取证**（2026-10-22 实测）：`User Data\Cache\artistEffect\<resource_id>\...\config.json` 会把类型写成 `InfoSticker`，据此已登记 21 个真实贴纸编号到 `user-enums.json`。缓存里**不写标题、分类和场景**，所以这些编号只能按 id 引用；`media harvest-enums add sticker <slug> <id>` 可继续补登记。
- 编号是 `resource_id`（16 位以上数字串），不是 `effect_id`，两者不要混用。
- 执行端**编号与显示名都接受**（2026-10-22 在 11.7 上逐类型实测：特效/滤镜/转场/字体/文本动画/蒙版均落地到草稿）；传编号时草稿里的显示名会保留那串数字，属外观问题不影响渲染。
- 表里的每一行都由脚本从 `catalogs/` 复制而来，`--check` 会拦住手工改动。

## 清单

该类型没有内嵌目录，见下面的采集方式。
