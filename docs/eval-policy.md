# 评估策略（`jianying-eval-policy/v1`）

`eval` 与 `loop` 的权重、扣分与阈值都可以外置成策略文件：

```bash
jianying eval draft/ --deep --policy policies/douyin.json --json
jianying loop draft/ --deep --policy policies/douyin.json --json
```

不传 `--policy` 时使用默认策略，**分数与历史行为完全一致**。
MCP 的 `jianying_eval` 也接受 `policy` 参数，Agent 可以按平台切换同一套阈值。

## 文件结构

```jsonc
{
  "schema": "jianying-eval-policy/v1",
  "name": "douyin",
  "weights":  { "main": 40, "continuity": 20, "caption": 20, "audio": 20 },
  "penalties": {
    "media-missing": 30, "source-exceeds-media": 15, "video-black": 25,
    "audio-silent": 20, "audio-mostly-silent": 10, "audio-loudness": 10
  },
  "thresholds": {
    "black_ratio_permille": 800,
    "silence_ratio_permille": 950,
    "mostly_silent_permille": 800,
    "loudness_min_lufs": -40,
    "loudness_max_lufs": -5
  }
}
```

缺省字段沿用默认值，因此可以只写要改的部分。

## 校验规则（不满足直接失败，不会"带病评估"）

| 规则 | 原因 |
|---|---|
| `weights` 均为有限非负数且总和 > 0 | 归一化分母必须有效 |
| `*_permille` 落在 `0..=1000` | 比例用千分比，越界即无意义 |
| `silence_ratio_permille >= mostly_silent_permille` | 否则"全静音"比"大部分静音"更容易触发，语义颠倒 |
| `penalties` 落在 `0..=100` | 单条问题扣分超过满分没有意义 |
| `loudness_min_lufs < loudness_max_lufs` 且均为有限值 | 否则区间为空 |

## 报告里会回显生效策略

```jsonc
{"score": 75, "issues": [...],
 "policy": {"name": "strict", "thresholds": {"black_ratio_permille": 150, ...}}}
```

这是**可复现性**的要求：同一个分数必须能追溯到"是哪套阈值算出来的"。

## 内置示例

- `policies/default.json`：与代码默认值一致，可作为改写起点。
- `policies/douyin.json`：竖屏短视频，允许更短的黑场但要求更响（`loudness_min_lufs: -38`）。
- `policies/strict.json`：成片交付前的严格自检（黑帧 100‰、静音 800‰）。
