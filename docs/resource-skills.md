# 资源技能：把官方素材变成可编排的 action

自动剪辑的前提不是“能执行命令”，而是**先能规划**：智能体要先写出一份类似剧本的 action 计划，
每个 action 说清用哪个素材、放在哪个位置、加什么效果。要做到这一点，模型必须在规划阶段就能查到
真实的资源编号，而不是凭记忆编。

这份文档说明仓库里那套“资源技能”是怎么来的、怎么用、边界在哪。

## 结构

```
.agents/skills/
  jianying-transition/   SKILL.md + references/transitions.md
  jianying-filter/       SKILL.md + references/filters.md
  jianying-effect/       SKILL.md + references/video_*.md
  jianying-text/         SKILL.md + references/fonts.md, text_animations_*.md
  jianying-audio/        SKILL.md + references/tone_effects.md, audio_scene_effects.md, ...
  jianying-sticker/      SKILL.md（无内嵌目录，见下）
  jianying-caption/      SKILL.md（无内嵌目录，见下）
```

- `SKILL.md` 是**契约**：什么时候用、怎么查、怎么绑到 `jianying-job/v2` 的哪个字段、有哪些坑。
- `references/*.md` 是**全量清单**：编号（`resource_id`）、名称、`effect_id`、是否 VIP、时长/参数。
  清单不进 SKILL.md，避免一次加载几千行。
- 两者都由 `tools/gen_resource_skills.py` 从 `catalogs/*.json` 生成；`--check` 会在 CI 里拦住手工改动。

## 规模

| 技能 | 清单行数 |
| --- | --- |
| jianying-transition | 453 |
| jianying-filter | 1052 |
| jianying-effect | 1337（场景 1097 + 人物 240） |
| jianying-text | 1133（字体 798 + 三套文本动画） |
| jianying-audio | 148（音色 57 + 场景音效 85 + 歌声转换 6） |

合计 4123 条，全部来自 pyJianYingDraft 的内嵌元数据（Apache-2.0，commit 见 `catalogs/index.json`）。

## 一次自动剪辑的形状

```json
{
  "schema": "jianying-job/v2",
  "project": {"name": "美食探店"},
  "operations": [
    {"op": "add_track", "type": "video", "name": "主画面"},
    {"op": "add_segment", "track": "主画面", "segment": {"material_id": "m1", "start_us": 0, "duration_us": 3000000}},
    {"op": "add_segment", "track": "主画面", "segment": {"material_id": "m2", "start_us": 3000000, "duration_us": 2000000,
      "transition": {"resource_id": "6724845717472416269", "duration_us": 500000}}}
  ]
}
```

规划阶段的动作顺序：**查清单 → 选中编号 → 写 action（编号 + 时间 + 参数）→ 执行 → 用证据校验**。
资源技能负责前两步，Job v2 负责后两步。

## 边界（必须知道的两件事）

1. **贴纸与字幕样式没有内嵌目录**。贴纸是应用侧在线素材，编号要用
   `jianying media harvest-enums` 从应用写出的真实 id 采集；字幕样式由“字体 + 文本动画 + 位置”组合而成，
   文本先用 `jianying media transcribe` 得到 SRT，再逐个写 `textSegment`。
2. **VIP 素材需要专业权益**。清单里标了 VIP 的行，执行时会被收据契约拦下
   （见 `docs/runbooks/evidence/official-resource/` 的拒绝矩阵与真实素材链路）。

## 重新生成

```bash
python tools/gen_resource_skills.py           # 写入
python tools/gen_resource_skills.py --check   # CI 用：不一致就失败
```

## 执行前的一道闸：`plan_resource_lint`

规划出来的 action 不能直接扔进执行器。`tools/plan_resource_lint.py` 在计划与执行之间做三件事：

```bash
python tools/plan_resource_lint.py plan.json --entitlement evidence.json --json
```

| 检查 | 命中时 |
| --- | --- |
| 编号是否存在 | `unknown_id`（带 JSON 路径，例如 `$.tracks[0].segments[0].transition.resource_id`） |
| VIP 素材是否有专业权益 | `vip_without_entitlement`（没传 `--entitlement` 时按无权益处理） |
| 时间字段是否合理 | `negative_start` / `non_positive_duration` |

索引来自 `catalogs/`，当前可解析 **4529 个 resource_id / 4541 个 effect_id**。退出码 0 表示这份计划可以交给执行器；
它不是只读草稿校验的替代品，只是把“素材选错”这类问题挡在执行之前。

### 已知缺口（2026-10-22）

`jianying job run` 目前**不校验** `resource_id` 是否存在：一个 `9999999999999999999` 的贴纸段能构建成功并产出草稿
（复现见 `docs/runbooks/evidence/official-resource/UNVALIDATED_RESOURCE_ID_20261022.json`）。在执行前请先跑
`tools/plan_resource_lint.py`。校验若要进产品，需要区分三类：滤镜/特效有目录可查，转场按 `name` 也可查，
而贴纸的编号来自应用侧、本来就不在目录里，不能一刀切。

### 2026-10-22 更新：上面那条「已知缺口」已经收敛

原文说 `jianying job run` 不校验 `resource_id`、随便一个 `9999999999999999999` 也能构建成功。这条**已不成立**：

- **解析层**：`catalogs::resolve` 现在同时接受显示名与数字 `resource_id`（此前只认名字，编号会被当成未知名字）；
  回归测试 `tests/job_cli_contract.rs::filter_and_effect_segments_accept_the_numeric_resource_id`。
- **段级校验**：滤镜/特效段的 `resource_id` 必须在目录里，否则 `is not in the bundled catalog`；
  转场、字体、文本动画、蒙版走同一条解析路径，六类逐一实测落地（`RESOURCE_ALIGNMENT_MATRIX_20261022.json`）。
- **贴纸**：没有内嵌目录，只做形状校验（非空、≤64 字符、仅 `[A-Za-z0-9-_.:]`），靠
  `media harvest-enums add sticker` 登记后再引用；测试 `add_registers_a_sticker_id_and_refuses_a_blank_one`。

### 三个与之配套的工具（都在 `tools/`，都有测试）

| 工具 | 用途 | 关键行为 |
| --- | --- | --- |
| `plan_resource_lint.py` | 查计划**或**草稿的资源引用 | `--draft <dir>` 读 `draft_content.json`；加密草稿报 `draft_content_encrypted` 而不是抛栈；能识别 `masks`/`fonts`/`material_animations` 等素材桶 |
| `prepare_editor_copy.py` | 给剪映准备一份可打开的副本 | 复制目录、把素材绝对路径改指副本自己的 `assets/`、写回明文；加密源直接拒绝 |
| `export_reconciliation.py` | 成片与草稿对账 | 省略 `--export` 时自动找最新 `.mp4/.mov`；比对时长（默认 120 ms 容差）、宽高比、帧率；代理渲染用 `--duration-source video` |

另外 `render proxy` 的返回现在带 `unsupported_features`、`skipped_material_counts`、`draft_has_unrendered_features`
与 `fps_source`——调用方可以机读地知道「这次渲染漏了转场/特效/蒙版」和「帧率是草稿给的还是回退的」。
