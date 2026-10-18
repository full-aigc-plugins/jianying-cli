# pyJianYingDraft 示例对照

上游 `GuanYixuan/pyJianYingDraft`（Apache-2.0，commit `6bb2d06`）**没有** `examples/` 目录，
可运行的示例是 `demo.py` 与 `tests/test_*.py`。这些文件已**逐字复制**到
[`upstream/`](upstream/)，作为对照的事实来源；我们这边的等价物与比对方式登记在
[`CASES.json`](CASES.json)。

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
```

## 怎么跑

```bash
# 1) 清单与上游对账（上游新增用例时会失败，逼你登记）
python tools/pyjyd_examples_manifest.py \
  --pyjyd-src <pyJianYingDraft 检出> --check

# 2) 参照构建 + 我们的构建 + 语义比对
python tools/pyjyd_examples_parity.py \
  --pyjyd-src <pyJianYingDraft 检出> \
  --bin target/release/jianying.exe
# → provenance/PYJYD_EXAMPLES_PARITY.json
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

## 未覆盖的用例

`template_loading`、`track_append_api`、`track_insert_api`、`track_basics`、`time_util`
是**行为型**示例：它们断言的是 API 语义（拒绝重叠、返回引用、时间解析取整），
不产出可比的草稿整体。它们在 `CASES.json` 里绑定到对应的 CLI 命令与测试文件，
比对方式是"同一规则在本仓库是否存在且被测试覆盖"。
