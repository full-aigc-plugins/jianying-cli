# 来源、版权与可迁移范围

本文件是工程合规分诊，不构成法律意见。任何商业发布仍应由发布主体按其组织政策复核。

| 来源 | 固定版本 | 原始许可 | 使用方式 | 工程决定 | 允许范围 | 禁止范围 |
|---|---|---|---|---|---|---|
| pyJianYingDraft | `c3318066d964744e2bfc66f75c71745fe8cea52a` | Apache-2.0 | 已复制骨架资产、生成目录元数据、wire 差分参考 | 有条件通过 | 在保留许可证、版权、修改说明和适用 NOTICE 的前提下迁移 | 删除归属、把上游素材宣称为本项目原创 |
| capcut-cli | `49f70e3b07f1a236d45acb9b49a70c141dd1ee98` | MIT | 86 命令能力基线、行为与黑盒差分参考 | 通过 | 独立实现；若后续复制实质代码则保留 MIT 声明 | 无来源记录的逐文件复制 |
| jianying-headless | `bb1e72cfc2bbed71ba7587548cebc85330b3e29a` | 个人学习与非商业自定义许可 | 仅作为能力目标，自主设计与获授权的黑盒验收 | 阻断为实现来源 | 描述“已有草稿编辑、ASR、控制、原生导出”等目标能力 | 复制或翻译源码、测试、蓝图、资源、fixture、实现常量；未获书面许可的商业整合 |

## 逐文件规则

- `assets/*.json`：来自 pyJianYingDraft 固定提交，哈希集合记录在 `SOURCE_MANIFEST.json`。
- `catalogs/*.json`：由固定提交的 Apache-2.0 元数据生成，必须通过 `tools/gen_catalogs.py --check` 和来源哈希门禁。
- `tests/parity/scenarios/*.json`：本项目编写的合成输入；允许在 pyJianYingDraft 固定提交上运行差分，不允许从受限 headless 仓库导入 fixture。
- `tests/fixtures/pyjyd_wire/*.json`：固定 pyJianYingDraft 输出的规范化 wire 参考及其来源记录；仅来自本项目合成输入，集合哈希受 manifest 门禁保护。
- `src/**`、`crates/**`：本项目 Rust 实现。任何参考来源必须进入 manifest；受限 headless 文件不得成为输入。
- 后续 capcut-cli 差分 fixture 必须记录为本项目合成输入或 MIT 固定提交来源，新增文件时同步更新精确清单和集合哈希。

## 门禁结果口径

- 原始来源：3。
- 确认误报：0。
- 真实阻断：1 个来源作为实现输入被阻断，即 `jianying-headless`。
- 未决：0；若获得新的书面授权，必须新建审查记录，不能直接修改本结论。
- 自动证据：`cargo test --test provenance_guard` 同时覆盖真实清单、未知来源和受限 fixture 负例。
