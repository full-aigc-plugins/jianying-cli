# pyJianYingDraft 示例对照

上游 `GuanYixuan/pyJianYingDraft`（Apache-2.0，commit `6bb2d06`）**没有** `examples/` 目录，
可运行的示例是 `demo.py` 与 `tests/test_*.py`。这些文件已**逐字复制**到
[`upstream/`](upstream/)（含 README 与 pypi_readme 的用法片段索引），作为对照的事实来源；
我们这边的等价物与比对方式登记在 [`CASES.json`](CASES.json) 与
[`README_SNIPPETS.json`](README_SNIPPETS.json)。

## 目录

```
examples/pyjyd/
  upstream/                 上游文件的逐字副本（含 LICENSE）
    demo.py
    tests/*.py
  cases/<id>/               每个可产出草稿的示例：参照实现 + 我们的 plan
    case.json               示例元数据、素材生成参数、已批准差异
    ref.py                  用 pyJianYingDraft 复刻上游示例的参照构建器
    plan.json               等价的 jianying-cli-plan/v1
  CASES.json                11 个用例 / 51 个上游测试函数的清点与映射
  README_SNIPPETS.json      32 段 README 代码片段的编号、sha256 与去向
```

## 怎么跑

```bash
# 1) 清单与上游对账（上游新增用例/片段时会失败，逼你登记）
python tools/pyjyd_examples_manifest.py \
  --pyjyd-src <pyJianYingDraft 检出> --check
python tools/pyjyd_readme_snippets.py \
  --pyjyd-src <pyJianYingDraft 检出> --check

# 2) 草稿型示例：参照构建 + 我们的构建 + 语义比对
python tools/pyjyd_examples_parity.py \
  --pyjyd-src <pyJianYingDraft 检出> \
  --bin target/release/jianying.exe
# → provenance/PYJYD_EXAMPLES_PARITY.json

# 3) 行为型示例：同一输入喂两边，比较结局
python tools/pyjyd_behavior_parity.py \
  --pyjyd-src <pyJianYingDraft 检出> \
  --bin target/release/jianying.exe
# → provenance/PYJYD_BEHAVIOR_PARITY.json
```

`pyjyd_examples_parity.py` 会用 ffmpeg 按 `case.json` 的 `media` 生成合成素材，
分别跑 `ref.py`（pyJianYingDraft）和 `jianying project build`，然后逐轨道比较：
片段起止、源窗口、音量、速度、关键帧通道、以及 `extra_material_refs` 指向的素材种类。

## 比对口径

* **数值**：起止/时长/音量/速度必须一致。
* **素材引用**：按"种类多重集"比较，而不是比个数；`case.json` 的
  `policy.approved_extra_kinds` / `approved_missing_kinds` 显式列出已批准的差异及理由，
  未列出的差异一律算失败。
* **已批准差异**：写理由，不写"容忍"。例如 pyJianYingDraft 的文本段会引用一个
  不存在于任何 materials 桶的 id（悬空引用），我们不复制这个缺陷。

## demo.py 对照中修掉的真问题

对照第一轮就暴露出四处偏差，都已修复：

| 偏差 | 上游行为 | 我们原来的行为 | 修复 |
|---|---|---|---|
| 花字素材类型 | `type = "text_effect"` | 写成 `text_shape` | 按气泡/花字分别写类型 |
| 文本段 `source_timerange` | 省略 | 与 target 相同 | 保留字段并在策略里登记（变更路径依赖该字段） |
| 视频段默认画布 | 不写 | 每段写 `canvases` + `material_colors` | 只在显式 `background_filling` 时写一个 canvas |
| 背景填充段的画布数 | 1 个 | 2 个（默认 + 模糊） | 同上 |

修复后 `demo-tutorial` 通过；随后 55 个 pyJYJ 场景差分仍为 55/55，
`provenance/PARITY_RUN.json` 逐字节未变。

## README 用法片段

上游 `README.md` 有 31 段代码块、`pypi_readme.md` 有 1 段，全部编号登记在
`README_SNIPPETS.json`：每段带行号、行数、sha256 与 `maps_to`。分类口径：

| 去向 | 数量 | 说明 |
|---|---|---|
| `case:*` | 12 | 直接对应某个草稿对照用例 |
| `behavior:*` | 16 | 绑定到 CLI 命令与测试文件的行为规则 |
| `reference:resource-ids` | 1 | 素材 id 清单（贴纸/气泡/花字） |
| `meta:install` | 2 | `pip install`，Python 侧安装说明 |
| `out_of_scope:controller-export` | 2 | `JianyingController` 导出，需客户端常驻 |

出现 `unmapped` 时门禁失败；`out_of_scope` 必须带理由（见 `out_of_scope_reasons`）。

## 行为型示例

`time_util`、`track_basics`、`track_append_api`、`track_insert_api`、`template_loading`
共 34 个测试函数断言的是 API 语义（时间解析、重叠拒绝、轨道顺序、模板轨道列举），
不产出整份草稿。它们由 `behavior_cases.py` 的 9 个用例**实际执行**两边后比较结局：

| 用例 | 比较内容 | 结果 |
|---|---|---|
| `time-parsing` / `time-rounding` | `tim()` 三个单位、`trange_seconds` 的 duration | 一致 |
| `time-negative` | `tim("-0.5s")` 与 `timeline move -- -500ms` | 一致（本轮修了 CLI 不接受负号） |
| `track-overlap` | 重叠与 1µs 边界：双方都拒绝 | 一致 |
| `track-type` | 视频素材进音频轨 | 已批准差异：我们只取音轨 |
| `track-end` / `append-order` | 轨道顺序与结束时间 | 已批准差异：主视频轨强制首位 |
| `insert-order` | `insert_track(over_track=...)` 的块顺序 | 一致 |
| `template-tracks` | 模板草稿的导入轨道列举 | 一致 |

已批准差异同样写理由，并且只对**指定的比较键**生效（`approved: {key: reason}`）。
