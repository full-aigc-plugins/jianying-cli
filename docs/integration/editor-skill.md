# 整合 jianying-editor-skill

上游：[`jianying-editor-skill`](../../research/jianying-editor-skill)（Python `JyWrapper` 封装 +
12 条规则 + 14 个脚本）。本仓库不复制其代码或资产，只做**能力对齐**：
把它的每个公开 API / 脚本映射到本 CLI 的真实命令，并对缺口如实登记。

## 覆盖表

| 上游 | 本 CLI | 状态 |
|---|---|---|
| `JyProject(name, w, h, root, overwrite)` | `project build` | covered |
| `save()` | `project build`（构建即落盘 + verify） | covered |
| `get_track_duration()` | `timeline tracks` | covered |
| `add_media_safe()` | `media add-video` | covered |
| `add_audio_safe()` | `media add-audio` | covered |
| `add_clip()` | `timeline add-segment` | covered |
| `add_text_simple()` | `captions add` | covered |
| `add_tts_intelligent()` | `media tts` | covered |
| `add_narrated_subtitles()` | `media tts` + `captions import-srt` | partial（缺旁白-字幕自动对齐） |
| `add_effect_simple()` | `timeline add-effect` | covered |
| `add_transition_simple()` | `timeline transition` | covered |
| `add_cloud_media()` / `add_cloud_music()` | — | out_of_scope（云端需要账号与签名） |
| `add_web_asset_safe()` | — | out_of_scope（需无头浏览器） |
| `draft_inspector.py` | `store list` + `project inspect` + `media material` | covered |
| `smart_rough_cut.py` | **`timeline cut-silences`** | covered |
| `auto_exporter.py` | `render native` + `job run` | partial（原生导出受宿主门禁） |
| `movie_commentary_builder.py` | `generate` | partial（缺解说文案生成） |
| `universal_tts.py` / `asset_search.py` / `api_validator.py` / `sync_jy_assets.py` | `media tts` / `media catalog` / `project verify` / `media harvest-enums sync` | covered |
| 录屏类（`web_recorder.py` / `smart_zoomer.py`） | — | out_of_scope（需屏幕录制管线） |

统计：27 个条目中 **17 covered / 3 partial / 8 out_of_scope**。
机器可读版本见 [`provenance/EDITOR_SKILL_COVERAGE.json`](../../provenance/EDITOR_SKILL_COVERAGE.json)。

## 新增能力：`timeline cut-silences`

上游 `smart_rough_cut.py` 的确定性对应物——按静音段剪掉停顿：

```bash
jianying timeline cut-silences draft --track audio \
  --min-silence 600ms --keep-pad 100ms --json
```

- 逐段探测素材静音（`media silence` 同一套 `silencedetect`）
- 只保留有声部分，两侧各留 `keep-pad` 作气口
- 删除量按**时间线时间**换算，其它轨道（字幕等）整体 ripple 前移
- 结果报告 `removed_us` / `kept_segments`，不改保留部分的时长

实测：2s 有声 + 1.5s 静音 + 2s 有声 → 剪掉 1.3s（留 100ms 气口），
音频变成 `0+2.1s / 2.1s+2.1s`，4s 处的字幕随之前移，`project verify` 通过。

## 契约与校验

- 映射文件：[`provenance/EDITOR_SKILL_MAPPING.json`](../../provenance/EDITOR_SKILL_MAPPING.json)
- 校验工具：`python tools/editor_skill_coverage.py --skill-src <检出> --out provenance/EDITOR_SKILL_COVERAGE.json`
  - 上游每个公开方法 / 脚本必须被映射，反向也不允许出现幽灵条目
  - `ours` 里出现的命令必须存在于 `jianying commands --json`
  - `partial` 必须写 `missing`，`out_of_scope` 必须写原因，`covered` 的证据文件必须存在
- 合同测试：`tests/editor_skill_coverage_contract.rs`（不依赖上游检出，只校验已提交的产物）

## 为什么不做的事

| 不做 | 原因 |
|---|---|
| 云端素材 / 曲库 / 花字库 | 需要账号、签名与网络，属鉴权红线，本 CLI 从不代为主张 |
| HTML→视频、录屏、录屏变焦 | 需要浏览器与屏幕录制管线，超出"无客户端剪辑"边界 |
| 复制其 Python 实现 | 许可与可移植性问题；能力对齐即可，代码保持独立 |

## 与 pyJianYingDraft 的关系

两者是**不同层**的对照：pyJYD 是草稿**数据层**（API 面 68/68 已对齐），
editor-skill 是**工作流层**（本文）。后者的大量能力（导入、字幕、特效、导出）
由同一批 CLI 命令承接，因此 `docs/vision` 里的 V4 可以同时引用两份真相源。
