# jianying-cli

**本地 Rust CLI：把 `jianying-cli-plan/v1` 计划一键构建为剪映专业版/CapCut 原生草稿——全能力域覆盖、构建即校验、可发布进草稿库。无账号、无上传、确定性输出。**

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)

`full-aigc-plugins` 生态的剪映引擎。三源整合（许可见 THIRD_PARTY_NOTICES.md）：

| 来源 | 许可 | 整合方式 |
|---|---|---|
| [GuanYixuan/pyJianYingDraft](https://github.com/GuanYixuan/pyJianYingDraft) | Apache-2.0 | 骨架资产 + **全量能力目录**（16 域 4400+ 条中文名→resource_id/effect_id 元数据，`catalogs/`，由 `tools/gen_catalogs.py` 生成）+ 落盘线型 |
| [renezander030/capcut-cli](https://github.com/renezander030/capcut-cli) | MIT | 双镜像写盘、root_meta_info 注册、编辑器守卫、app 骨架种子、draft_materials 注册、代理渲染纪律（事实与行为对齐，无代码复制） |
| [partme-ai/jianying-headless](https://github.com/partme-ai/jianying-headless) | 个人学习与非商业 | 仅契约事实（build→verify→publish 门禁、主轨连续规则）；零代码零资产 |

## 能力矩阵

- **轨道**：video（多轨=画中画）、audio、text、sticker、filter（全局）、effect（全局）
- **视频段**：变速(0.1-8)/音量(0-4)/裁剪/缩放位移旋转不透明度、线性关键帧（scale/x/y/rotation/opacity）、**转场（453 目录）**、**蒙版（6 形，羽化/反相/圆角）**、**滤镜（1052）**、**画面/人物特效（1097+240，参数 0-100）**、**混合模式（10）**、**入场/出场/组合动画（155+124+123）**、图片段
- **音频段**：**淡入淡出**、**场景音/音色/声音成曲（85+57+6）**、音量关键帧
- **文本段**：字体（798 目录）、描边、阴影、背景条、**多范围样式（UTF-16 偏移）**、入场/出场/循环动画（145+97+93）、花字（raw id 直通）
- **工程**：双镜像写盘、meta 侧车、`draft_materials` 媒体注册、app 骨架种子（`--seed`）、`root_meta_info.json` 注册、剪映运行守卫、结构 lint、ffmpeg 代理渲染、SRT 字幕注入

## 安装与使用

```bash
cargo build --release          # 产物 target/release/jianying

jianying doctor                                       # 环境/草稿根/编辑器预检
jianying status --json                                # 环境、能力与持久任务健康摘要
jianying audit [TASK_ID] --json                       # 全局或单任务追加式审计视图
jianying completion zsh > _jianying                   # 生成补全脚本，不修改 shell 配置
jianying project build plan.json --out d1 [--srt subs.srt] # 构建（自带 verify 门禁）
jianying project verify d1 && jianying project inspect d1  # 结构 lint / 摘要
jianying store publish d1 [--root DIR] [--force]           # 发布进草稿库并注册
jianying render proxy d1 --burn-captions                   # ffmpeg 代理预览（非成片）
```

## 四种建草稿的方式（命令式优先）

计划文件**不是必选项**。只做一次剪辑时，直接敲命令即可，这也是最接近
pyJianYingDraft 逐句 API 调用的用法：

```bash
# ① 命令式：建成空草稿，再逐条加内容（推荐作为默认路径）
jianying project init "我的片子" --out draft --width 1080 --height 1920
jianying media add-video draft ./clip.mp4 0s 4s          # 加视频（带音频会自动混入）
jianying captions add draft "第一句字幕" 0s 2s            # 加字幕
jianying timeline speed draft <segment_id> 1.2            # 改已有片段
jianying project verify draft && jianying render proxy draft --out film.mp4

# ② 一条命令：把素材/字幕直接变成草稿
jianying project quickstart "快速" --out draft --video ./clip.mp4 --audio ./bgm.mp3

# ③ 声明式：从计划文件构建（需要版本化、可回滚、可组合时）
jianying project build plan.json --out draft

# ④ 兼容 capcut-cli 的完整规格
jianying project compile spec.json --out draft
```

没有任何一步需要剪映客户端；`ffmpeg` 只在探测、渲染、深度评估时用到。

## 什么时候才需要"计划"

| 场景 | 用命令式 | 用计划 |
|---|---|---|
| 手工剪一段、改一个片段 | ✅ | 没必要 |
| 同一套结构批量出多条 | 命令会很长 | ✅ 模板化 + `generate` |
| 需要 diff / 回滚 / 组合 | 难追踪 | ✅ `plan compose/append/history/revert` |
| 交给 Agent 生成整条时间线 | 工具调用爆炸 | ✅ 一份 JSON 描述全片 |
| 需要跑评估-修复闭环 | 可以跑但不便于复现 | ✅ `eval` / `loop` 直接吃计划产物 |

计划的代价是多了一层模型；收益是可 diff、可回滚、可组合、可批量生成。
如果你的目标是"用 CLI 剪一段"，请直接用 ① 或 ②。

## 无客户端智能剪辑流水线

从素材到成片预览，全流程不需要剪映客户端：

```bash
# 1) 简报 → 计划（平台画布、目标时长、镜头切分、素材轮换）
jianying generate brief.json --out plan.json
# 2) 计划 → 草稿（组合、增量、回滚同样在计划层完成）
jianying project build plan.json --out draft
# 3) 评估 → 修复 → 重评（结构/素材/画音/镜头节奏四类问题）
jianying eval draft --deep --json
jianying loop draft --deep --policy policies/strict.json --json
# 4) 出片：代理预览 + 单帧抽检
jianying render proxy draft --out film.mp4
jianying render frame draft --at 1500ms --out frame.png
```

计划可组合与回滚：`jianying plan compose/append/history/revert`。
评估阈值可按平台调优：`policies/{default,douyin,strict}.json`，见
[`docs/eval-policy.md`](docs/eval-policy.md)。

## MCP（stdio）

```bash
jianying mcp serve     # 17 个工具：8 个 job、3 个命令桥、5 个检视/闭环、1 个抽帧
```

Agent 既可以用 `jianying_command_run` 调任意 CLI 命令，也可以用
`jianying_draft_inspect` / `jianying_timeline_segments` / `jianying_eval` /
`jianying_loop` / `jianying_render_frame` 做结构化读写。
详见 [`docs/mcp-server.md`](docs/mcp-server.md)。

## 目标达成证据

`provenance/GOAL_SNAPSHOT.json` 是按需重新生成的机器快照（commit、自检矩阵、
命令/能力/技能数量、两份覆盖摘要）。重新生成：

```bash
python tools/goal_snapshot.py --binary target/debug/jianying.exe
```

官方客户端协议为何不实现：见 [`docs/adr/ADR-003-official-protocol-out-of-scope.md`](docs/adr/ADR-003-official-protocol-out-of-scope.md)。

## 一条命令自检

拿到二进制后先跑自检：它按目标的六条验收线逐项检查，缺依赖的项如实标 SKIP。

```bash
jianying selftest --json
# {"ok":true,"summary":{"total":7,"pass":7,"fail":0,"skip":0}, ...}
```

| 检查项 | 验证什么 |
|---|---|
| `capabilities` | 能力清单可读、状态合法 |
| `skills` | 工作流技能注册表（14 条） |
| `mcp` | MCP 工具面（17 个） |
| `plan` | 计划组合 + 追加 + 回滚 |
| `draft` | 草稿构建并通过结构校验 |
| `media` | 素材探测与深度评估（需 ffmpeg） |
| `render` | 代理渲染 + 单帧导出（需 ffmpeg） |

## 发布物自校验

```bash
jianying release verify <bundle-dir> --json   # 逐文件 sha256/大小 + 能力契约比对
```

发布流程与授权边界见 [`docs/release-checklist.md`](docs/release-checklist.md)。

计划契约、全部字段与校验规则见 [`docs/plan-format.md`](docs/plan-format.md)。
计划用 `allow_vip: true` 显式声明后才允许引用 VIP 目录成员（会员权益是
用户自己的授权，本 CLI 从不代为主张）。

## 智能体接口规范

命令面、退出码、JSON 输出契约与全部约束见
[`docs/agent-interface.md`](docs/agent-interface.md)（事实源）；`jianying --help`
同步内置。能力名检索：`jianying media catalog [--domain <d>] [--search <名>]`。
`build/verify/inspect/probe/catalog/publish` 与旧式 `render <dir>` 暂作兼容别名，
调用共享 handler 并在 stderr 输出迁移提示；`jianying commands --json` 提供机器可读替代关系。

## 边界（如实）

- 代理渲染不渲染转场/特效/蒙版——权威出口是剪映内导出。
- 已有草稿的媒体/时间线编辑已进入 Rust CLI；ASR 已具备官方 whisper.cpp CLI 的本地结构化
  adapter 与幂等账本，但真实模型 canary、字幕写回和云端 ASR 尚未验收；原生 MP4 导出仍受
  真实剪映宿主门禁约束。不导入非商业 headless 项目的实现代码或资产。
- 曲线变速、新版剪映加密草稿读取：上游三项目同样不支持。

## 双源验证测试集

`tools/parity_run.py` 是差异测试闭环：每个场景的同一计划分别由
pyJianYingDraft（参考实现，`tools/parity_reference.py`）与 jianying-cli
构建，再语义化比对产物（忽略 id/时间戳/路径，数值归一，引用解析到桶，
材料按 ref⊆cli 子集匹配）。55 个场景（pyJYD 作者域全覆盖：20 组合动作 + 逐域多样性 + change_pitch/crop/srt 参数面/插轨）覆盖全部能力域——转场（默认/自定义
时长）、蒙版（圆形/矩形+圆角+反相）、滤镜强度、特效参数、混合模式、
出入场动画、片段原声淡入淡出、音频淡入淡出、场景音、字体/描边/阴影/
背景条、多范围样式（CLI 超集）、色度抠图、画布背景填充、关键帧、
全局滤镜/特效轨、时间字符串（tim() 语法）。

```bash
cargo build --release
cargo test                                     # 349 unit/contract tests
python tools/parity_run.py                     # 55/55 PASS
python tools/gen_catalogs.py <pyJYD检出> --check  # 目录与 pyJYD metadata 一致

> **Windows 上跑测试前先确认 Python 解释器**：`provenance_guard` 等门禁会 `shell out` 到 Python。
> Microsoft Store 的 `python3.exe` 是可启动但立刻失败的占位别名（退出码 9009），门禁会误报失败。
> 设 `PYTHON` 指向真实解释器即可（现在是可操作提示，不再只打印占位提示）：
>
> ```powershell
> $env:PYTHON = (Get-Command python.cmd).Source   # 或任意真实 python.exe
> $env:JIANYING_CLI_PROCESS_LIST = "$env:TEMP\empty.txt"   # 本机 tasklist 卡死时用它跳过"剪映在运行"检查
> cargo test -p jianying-cli
> ```
```

历史上这套测试抓出并修正了：border/阴影的精确数值映射（×0.002、/600）、
text_shape 类型名、scale 关键帧只写 ScaleX + 关 uniform_scale、文本段
默认 transform_y=-0.78 与 global_alpha、音频特效 sub_type/time_range、
transition 默认时长取目录值等 10+ 处偏差。

## 下游

[`partme-ai/partme-jianying-plugin`](https://github.com/partme-ai/partme-jianying-plugin)
以 vendored 快照消费本仓（lock 钉扎 + 自动同步 PR），技能家族见
[`full-aigc-skills/jianying-skills`](https://github.com/full-aigc-skills/jianying-skills)。
