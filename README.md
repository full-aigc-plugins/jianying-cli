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
jianying build plan.json --out d1 [--srt subs.srt]    # 构建（自带 verify 门禁）
jianying verify d1 && jianying inspect d1             # 结构 lint / 摘要
jianying publish d1 [--root DIR] [--force]            # 发布进草稿库并注册
jianying render d1 --burn-captions                    # ffmpeg 代理预览（非成片）
```

计划契约、全部字段与校验规则见 [`docs/plan-format.md`](docs/plan-format.md)。
计划用 `allow_vip: true` 显式声明后才允许引用 VIP 目录成员（会员权益是
用户自己的授权，本 CLI 从不代为主张）。

## 边界（如实）

- 代理渲染不渲染转场/特效/蒙版——权威出口是剪映内导出。
- 原生 MP4 导出、已有草稿编辑、ASR 记账在 NC fork 专业档（`partme-jianying-plugin` 的 `jianying-harness`），因许可不在本 CLI。
- 曲线变速、新版剪映加密草稿读取：上游三项目同样不支持。

## 下游

[`partme-ai/partme-jianying-plugin`](https://github.com/partme-ai/partme-jianying-plugin)
以 vendored 快照消费本仓（lock 钉扎 + 自动同步 PR），技能家族见
[`full-aigc-skills/jianying-skills`](https://github.com/full-aigc-skills/jianying-skills)。
