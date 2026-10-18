# 目标态：无客户端智能剪辑

这份文档把"最终要变成什么"写成可验收的条目，并给出每一项的**当前证据**。
凡是没跑过的，一律标注"未验证"；凡是做不到的，直接写进"边界"。

## 1. 六条验收条目

| # | 用户要求 | 可观测的验收方式 |
|---|---|---|
| V1 | 完全替代 pyJianYingDraft | pyJYD 的 API/字段映射 `uncovered=0`；55 个双源场景 55/55；运行时零 Python 依赖 |
| V2 | 具备 dream-loop 的设计思想 | 生成-评估-修正的闭环可运行，且每轮有可复核的指标与停止条件 |
| V3 | 具备 hypit 的设计思想 | 计划层可被程序化改写/组合，且改动可解释、可回滚 |
| V4 | 整合 jianying-editor-skill | 技能以可调用命令/工具形式暴露，有调用契约与失败语义 |
| V5 | 整合 jianying-headless 能力 | 无需交互式客户端即可批量出片（渲染、字幕、素材处理） |
| V6 | **没有剪映客户端也能完成智能剪辑** | 一条命令从"素材 + 意图"产出成片文件，全程不启动剪映 |

## 2. "无客户端"的准确边界

客户端在三个位置可能被需要，必须分开看：

```mermaid
flowchart LR
  M[素材] --> P[计划 plan/v1]
  P --> D[草稿 draft_content.json]
  D --> R1[本工具 ffmpeg 代理渲染]
  D --> R2[剪映内导出]
  R1 --> O1[MP4 预览级]
  R2 --> O2[MP4 权威级]
  D -.->|可选| C[剪映客户端]
```

- **构建与编辑**：不需要客户端（纯文件读写 + 结构校验）。✅ 已实现
- **预览出片**：不需要客户端，用 ffmpeg 平铺主轨 + 混音 + 可选烧字幕。✅ 已实现（**预览级**）
- **权威导出**：仍需要客户端。⚠️ 明确不承诺

`render proxy` 自述的边界是"不含转场/特效/蒙版；权威出口是剪映内导出"。这条边界是设计选择，
不是缺陷——把预览级渲染伪装成权威导出才是缺陷。

## 3. 端到端实测（无客户端）

```
素材：ffmpeg 生成 3s 640x360 h264 + 3s aac
计划：video(0-3s) + text("无客户端出片")
命令：project build → render proxy
结果：preview.mp4 = h264 320x180 / 3.000000s / 3975 bytes / 0.15s
```

`render proxy` 返回：`video_tracks_flattened=1`、`audio_segments_mixed=0`（该计划无音轨）、
`captions_burned=0`、`target_fps=30`。全过程未启动剪映。

复现：

```bash
ffmpeg -f lavfi -i color=c=0x1E90FF:s=640x360:d=3 -f lavfi -i sine=frequency=440:duration=3 \
       -c:v libx264 -pix_fmt yuv420p -c:a aac -shortest clip.mp4
jianying project build plan.json --out draft --json
jianying render proxy draft --out preview.mp4 --json
ffprobe -v error -show_entries format=duration:stream=codec_name,width,height -of json preview.mp4
```

## 4. 六根支柱现状矩阵

| 支柱 | 现状 | 证据 |
|---|---|---|
| V1 pyJYD 替代 | 🟢 API 面 **68/68 类、90/90 方法全部登记**（62 covered + 6 类 L3 out_of_scope，`partial=0`、`uncovered=0`）；55/55 双源差分仍为行为证据 | `provenance/PYJYD_API_COVERAGE.json`、`tests/pyjyd_api_coverage_contract.rs` |
| V2 dream-loop 闭环 | 🟢 闭环四类问题可检出、三类可自动修：结构（空洞/空轨）、素材（越界收窄/缺失告警）、画音（黑帧/静音/响度）、节奏（长镜等分、碎切告警）；阈值与权重由 `jianying-eval-policy/v1` 外置；CLI 与 MCP 共用同一实现 | `src/eval_ops.rs`、`src/eval_policy.rs`、`tests/eval_loop_contract.rs`、`tests/eval_policy_contract.rs`、`tests/brief_generate_contract.rs` |
| V3 hypit 计划层 | 🟢 计划可编排：`plan compose/append` 确定性拼接（画布一致、主轨优先、按累计时长偏移），每次写入留快照，`plan history/revert` 逐字节回滚 | `src/plan_ops.rs`、`tests/plan_composition_contract.rs` |
| V4 editor-skill 整合 | 🟡 CLI 侧完成：上游 27 个公开 API/脚本中 17 covered、3 partial、8 out_of_scope，逐条映射到真实命令并有合同测试；17 个 MCP 工具、发布物可自校验；**差最后一步**：tag 构建与插件发版需要授权 | `docs/integration/editor-skill.md`、`provenance/EDITOR_SKILL_COVERAGE.json`、`tests/editor_skill_coverage_contract.rs` |
| V5 headless 能力 | 🟢 批量流水线已通：`pipeline <计划目录>` 一次完成 构建→评分→渲染，失败按条隔离，`--fail-under` 作 CI 门禁；**渲染仍是预览级** | `src/pipeline_ops.rs`、`tests/pipeline_contract.rs` |
| V6 无客户端出片 | 🟢 **预览级已通**（§3 实测）：`generate → build → eval → loop → render proxy / render frame` 全链路可跑；权威导出明确不做 | 本文 §3 |

图例：🟢 已达成 / 🟡 部分 / 🔴 未开始。

## 5. 已知阻塞与决定

| 项 | 状态 | 说明 |
|---|---|---|
| `speed_curve`（曲线变速） | **阻塞** | pyJYD 只写 `curve_speed: null`，没有格式参考；剪映 11.6 草稿加密，拿不到真实样本。不做猜测实现 |
| `compound/member`（复合片段） | **阻塞** | 同上，缺明文样本 |
| 草稿解密 | **不做** | 属客户端保护机制，越界 |
| 联网素材目录 | **不做** | 需签名与登录态；改由 `resource mine/import/apply` 复用用户素材 |
| 官方 agent 通道 | **只留适配位** | 需 `JIANYING_AGENT_RUNTIME_TOKEN`，不自取令牌 |

## 6. 下一步（按价值排序）

1. **V2 闭环**：定义 `eval` 指标（时长/黑帧/响度/字幕覆盖率）+ `loop` 命令（生成→评估→修正，带停止条件与审计）。这是"智能"二字的落点。
2. **V4 技能整合**：把 `docs/reference` §3.3 的 19 个技能映射为插件内的可调用能力声明，缺的标 `partial`。
3. **V5 批处理**：`render batch` 已有，补一条"多片一次出 + 汇总验收"的端到端用例。
4. **V1 收尾**：pyJYD API 面清单（`uncovered=0` 的可计算版本）。

> 说明：本文的"未验证"指没有在本仓库留下可复现命令与输出的结论。

## 7. 评估-修正闭环实测（2026-10-18）

```
project build（两段连续主轨）→ eval              → score 100, issues []
timeline move 第二段 +2.5s      → eval              → score 62, issues [main-track-gap], gap_us 2.5s
                                → loop --min-score 100
                                  round 1: close-main-track-gaps  62 → 100
                                  final: score 100, gap_us 0, stopped "clean"
```

`eval` 前后 `draft_content.json` 字节一致（只读）；`loop` 的每次修复都走事务
（快照 → 校验 → 原子提交），失败不会留下半成品。

## 8. 第 23 轮复验（最终证据）

| 环节 | 命令 | 结果 |
|---|---|---|
| 生成 | `generate brief.json --out plan.json` | 平台画布、目标时长、`max_shot_us` 切分、`min_shot_us` 尾段合并、`shot_order=round-robin` 轮换 |
| 构建 | `project build plan.json --out draft` | 自带 verify 门禁 |
| 评估 | `eval draft --deep` | 结构 / 素材 / 画音 / 节奏四类，报告回显生效策略 |
| 修正 | `loop draft --deep --policy policies/strict.json` | 停止原因 `clean / below-target / no-safe-repair / max-rounds-reached` |
| 出片 | `render proxy` + `render frame` | 2s 预览含音轨；单帧 PNG 可肉眼校验 |
| 发布物 | `release verify <bundle>` | 11 文件哈希 + 能力契约 156/156 一致 |
| 文档 | `cargo test --test docs_command_contract` | README/文档里每条 `jianying …` 都真实存在 |

测试总量 **349**，clippy `-D warnings` 0，`cargo fmt --check` 0，`tools/check_provenance.py` 0。

## 9. 音乐卡点（第 24 轮）

`jianying media beats <audio> --json` 给出能量起点与 BPM 估计：

- 包络：ffmpeg `astats` 逐 1024 采样（23ms）RMS
- 起点：局部极大 + 相对前一帧 `min_rise_db`（默认 3dB）抬升
- BPM：相邻间隔中位数（对漏检首尾不敏感），另给 `bpm_span` 作参考
- 诚实边界：合成 120 BPM 点击轨估出 117（−2.1%），**持续正弦报 0 个节拍**；
  对铺底人声/弦乐会漏检，结果里的 `confidence` 会如实降级

技能注册表里 `beat-sync` 由 `missing` 升为 `partial`：检测可用，
"自动对齐重排"仍需在计划层指定切点。
