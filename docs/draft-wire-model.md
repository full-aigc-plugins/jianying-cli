# 草稿 wire model 与规范化差分

`jianying-draft` crate 提供 `draft_content.json` 和 `draft_meta_info.json` 的 Rust
wire model。它覆盖草稿元数据、画布、时间线、轨道、片段及微秒时间范围；资源对象在
保留完整 `serde_json::Value` 的同时进入强类型语义 inventory，样式与组合编辑字段继续
按 OpenSpec 4.4 收紧。

## 资源语义 inventory

`DraftResourceInventory` 将素材分桶和片段内联节点统一投影为十类资源：视频、音频、
文字、贴纸、滤镜、特效、转场、蒙版、动画和关键帧。每项资源保留原始 JSON，同时
提供稳定的 `id`、bucket、wire type 和 `DraftResourceKind`。

writer 落盘前会执行资源语义校验：

- 视频/图片、音频必须携带数值时长，文字必须携带可写回的 content；
- 贴纸、滤镜、特效、转场和蒙版必须携带相应 resource/effect 标识；
- 转场必须携带时长，蒙版必须携带 config；
- 动画容器必须含非空动画列表，每项声明类型、资源、起点和时长；
- 关键帧列表和点使用强类型 wire model，属性、点和值不可为空。

该层只证明资源对象的分类和自身语义。Python/Rust 每类资源的差分等级属于 4.5，不能由
本层替代。

## 引用闭包与草稿 bundle 门禁

`DraftReferenceIntegrity` 对所有素材 bucket 建立全局 ID 索引，拒绝跨 bucket 重复素材 ID、
重复轨道/片段 ID、主素材 bucket 不匹配和悬空 `extra_material_refs`。它会索引包括 speed、
canvas、fade 等 companion bucket 在内的全部素材，而不只索引 4.2 的十类强语义资源。

`DraftBundleIntegrity` 进一步验证：

- `draft_content.json` 与 `draft_info.json` 语义完全相等；目录校验还要求两个文件逐字节相同；
- 时间线 name/duration 与 `draft_meta_info.json` 一致；
- `draft_fold_path`、`draft_root_path`、`draft_json_file` 与实际草稿目录一致；
- 本地视频/音频文件存在、位于草稿目录内，并在 `draft_materials` 中以兼容类型注册。

模板 duplicate/import/build-on-template 和草稿库 publish 会在复制目录后重定位本地素材路径，
同步双时间线镜像与素材注册，再执行 bundle 校验。导入不再保留对源草稿目录的隐式依赖；
校验失败时不会把目标登记进草稿库。

## 编辑语义门禁

`DraftEditSemantics` 将此前只存在于 plan 校验与 writer 分支中的行为提升为可用于已有草稿的
wire 门禁：

- 字幕/文字 content 必须是含正文和非空 styles 的 JSON，样式区间按 UTF-16 code unit
  校验，字号和 alignment 受值域约束；
- 片段 speed/volume 与 speed companion 资源保持一致并满足剪映值域；
- 视频 crop 八点均为 0..1，clip alpha/rotation/scale/transform 和 uniform_scale 结构受约束；
- `mix_mode` 组合素材强度为 0..1，且必须由片段 `extra_material_refs` 实际引用。

新建、模板编辑、模板叠加、发布与 `verify` 均复用同一门禁，因此不再出现“plan 合法但模板
操作写出非法编辑字段”的两套规则。

## 无损边界

- 每层对象使用 `serde(flatten)` 保存未知字段；已存在的未知根字段、画布字段、轨道字段、
  片段字段和时间范围字段可反序列化后原样写回。
- 真实已有草稿的修改仍使用 `LosslessDraftEnvelope` 定点替换，避免“反序列化后整体重建”
  改变未触及字段。
- 新草稿 writer 在落盘前必须通过 `DraftTimelineWire` 与 `DraftMetadataWire` 解析，防止
  writer 输出偏离已迁移的 wire model。

## 规范化规则

跨 Python/Rust 的核心差分保留以下字段：

- 时间线：`duration`、`fps`、`canvas_config`、`tracks`；
- 轨道：`attribute`、`flag`、`is_default_name`、`name`、`type`、`segments`；
- 片段：`target_timerange`、速度、音量、视觉变换和统一缩放；
- 视频与音频片段额外保留 `source_timerange`，文字等非媒体轨道忽略该兼容差异。

规范化会排除随机 ID、时间戳、绝对路径、素材引用以及尚未纳入当前任务的资源语义。
被排除字段不是被判定为等价，而是由 4.2–4.5 的资源级差分继续收紧。

## 冻结参考证据

`tests/fixtures/pyjyd_wire/text_core.normalized.json` 由固定提交
`c3318066d964744e2bfc66f75c71745fe8cea52a` 的 pyJianYingDraft，针对本项目合成输入
`tests/parity/scenarios/17-text-styles.json#plan` 生成。对应来源与归一化规则记录在相邻的
`text_core.provenance.json`，两者共同受 `SOURCE_MANIFEST.json` 哈希门禁保护。

验证命令：

```bash
cargo test --test wire_model_differential --locked
cargo test -p jianying-draft --test resource_semantics --locked
cargo test -p jianying-draft --test reference_integrity --locked
cargo test -p jianying-draft --test edit_semantics --locked
cargo test --test ops_behavior --locked
python3 tools/check_provenance.py
```

动态差分通过 `tools/pymediainfo.py` 将固定上游所需的最小 MediaInfo API 映射到系统
`ffprobe`，无需把 Python 包或 MediaInfo 绑定带入 Rust 运行时。执行：

```bash
cargo build --release --locked
python3 tools/parity_run.py --report provenance/PARITY_RUN.json
python3 tools/check_provenance.py --self-test-negative-cases
```

`PROTOCOL_DIFFERENTIALS.json` 将 20 类协议对象映射到具体场景和比较等级；本轮 55/55
场景通过。默认使用去除随机 ID、时间戳和绝对路径后的规范化等价。多 UTF-16 样式区间是
明确批准的 Rust 超集差异，比较前只投影共同的 base style，并记录 rationale；没有把该扩展
伪装成完全相等。

边界 fixture 位于 `crates/jianying-draft/tests/fixtures/draft_edge_cases.json`，固定覆盖空轨、
重叠、负偏移、变速 companion、缺失资源、Unicode/UTF-16 和未知字段写回。重叠与负偏移在
读取已有草稿时无损保留，作者侧 plan 和目录级 `verify` 再按各自职责报告非法时间线；缺失
素材在引用闭包层直接失败。

CI 会重新生成 `/tmp/PARITY_RUN.json` 并与版本库报告逐字节比较，然后执行协议对象完整性和
负例门禁。只要任一场景失败、任一对象没有比较等级/证据，或批准差异缺少 rationale，
`protocol.pyjyd_wire=supported` 的发布声明就无法通过 CI。该能力只表示 wire 协议迁移门禁，
不表示 capcut-cli 86 项命令或原生运行时能力已经完成。
