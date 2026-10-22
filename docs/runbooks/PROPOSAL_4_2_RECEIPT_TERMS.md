# 提案：把「价格文件 / 授权文件 / 用户可见提示」纳入官方素材收据契约（4.2 收口所需）

## 为什么需要这份提案

4.2 要求证明"价格文件、政府授权与用户可见信息"的**拒绝路径**。当前收据契约
（`schemas/jianying-official-resource-receipt-v1.schema.json`）只表达**素材本身**：
`asset_id`、`identity`（`downloaded_file` / `visual_only` / `live_profile`）、`tier`、
`allowed_usages`、`restrictions`、`preview_only`、`terms_reference`。

它**没有**表达"价格"与"授权文件"的位置。因此 4.2 的另一半不是"再多测几遍"，而是**契约缺字段**——
这是产品决策，不是工程缺口。本提案给出三个可拍板的选项，选定后我可以直接实现 + 补测试。

## 现状（已实现的拒绝路径）

| 情形 | 现在的行为 | 证据 |
| --- | --- | --- |
| 过期 / basic 版 / 非 active / 用途不允许 / 内容被改 / 收据与文件不符 / `preview_only` | 全部失败关闭 | `REFUSAL_MATRIX_20261021.json`：八种情形七拒一过 |
| 真实素材 + 真实收据 | `verify_personal=verified`，`verify_commercial=asset_use_denied` | `REAL_ASSET_CHAIN_20261022.json` |
| 收据生命周期 | `verify → register → show → list → preflight` 闭环 | `RECEIPT_ROUNDTRIP_20261021.json` |

## 三个选项

### 选项 A（最小改动）：把条款与价格作为**引用**字段

在 `identity` 同级增加：

```json
"terms": {
  "reference": "string  // 指向条款文本的稳定标识，例如面板标注文本或文件 sha256",
  "price_reference": "string | null  // 价格文件的身份：sha256 或 URL",
  "authorisation_reference": "string | null  // 授权文件（含政府/机构授权）的身份",
  "user_visible_notice": "string | null  // 界面提示原文，例如 不可商用"
}
```

- 拒绝路径：`terms.reference` 为空 → `terms_missing`；`price_reference` 指向的文件 sha256 与声明不符 →
  `price_document_mismatch`；`authorisation_reference` 存在但文件缺失 → `authorisation_document_missing`。
- 优点：改动小，兼容现有收据（新字段可空）；能立刻把 4.1 的"不可商用标注"作为 `terms.reference` 落进契约。
- 缺点：它证明的是"引用与文件一致"，**不证明条款内容本身**。

### 选项 B（中等）：条款作为独立**文件型身份**

允许 `identity.kind` 增加 `terms_file` / `price_file` / `authorisation_file`，与素材并列，各自带 sha256 与用途。
收据可以同时引用一个素材与若干文件（`related_documents: [...]`）。

- 优点：文件本身可校验、可归档，最贴近"证据"语义。
- 缺点：需要扩展 schema 的 `identity` 联合类型，并让 `show/list/preflight` 都理解多文档。

### 选项 C（不推荐）：把条款文本内联进收据

- 优点：单文件自包含。
- 缺点：条款文本会随收据复制扩散（许可证文本再分发风险），且体积不可控。

## 我建议选 A

理由：
1. 4.1 刚刚以"`不可商用` 标注即许可证据"收口，A 能让同一个标注**直接**进入契约（`terms.reference` + `user_visible_notice`），
   口径一致；
2. 拒绝路径清晰可测（空引用、文件不符、文件缺失三类），能给出与 `REFUSAL_MATRIX` 同构的矩阵；
3. 不引入多文档联合类型，`show/list/preflight` 的语义不变，风险最低。

## 选定之后我会做的事（预计一次提交内完成）

1. 扩展 `schemas/jianying-official-resource-receipt-v1.schema.json`（按所选选项）；
2. `crates/` 侧增加对应字段与三类拒绝码，`fixtures/official-receipt-*.json` 同步；
3. `tests/` 增加拒绝矩阵（空/不符/缺失），并把 4.1 的真实收据升级为带 `terms` 的版本；
4. 更新 `verification.md` 与 4.2 任务备注，届时 4.2 才可勾选。

## 我需要的决策

一句话即可：**A / B / C**。若选 A，并请确认 `price_reference` 与 `authorisation_reference` 是否允许为空
（当前没有这两个文件，允许为空才能先把 4.1 的素材收据升级）。
