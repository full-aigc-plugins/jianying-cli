# Verification

## 2026-09-22 — pyJianYingDraft domain parity continuation

- Reconciled the OpenSpec task state with the authoritative parity matrix by adding open tasks 4.8–4.12. The four pyJianYingDraft rows remain `partial`; no completion claim was promoted.
- Added a machine gate that requires an `incomplete` source claim, an explicit gap and at least one unchecked OpenSpec blocking task for every partial pyJianYingDraft function or data structure. Negative self-test rejects a false completion claim.
- Migrated the first domain slice into dedicated Rust value objects: `ClipSettings`, `CropSettings` and `Transform`. Video, audio, text and sticker segments now validate and carry these objects while serde flattening preserves the existing Job v2 wire shape.
- The v1 converter now promotes speed, volume, pitch-change, eight-point crop and static visual transform into the unified domain; Domain-to-Plan projection restores the same fields without reading the compatibility payload.
- Updated the checked-in Job v2 JSON Schema so video, audio, text and sticker fields remain type-specific; audio cannot acquire visual fields through the schema.
- TDD RED: domain contract compilation failed because the three value objects and segment fields did not exist.
- TDD GREEN: targeted domain/provenance/job tests passed 33/33; `cargo clippy --workspace --all-targets -- -D warnings`, Python `ruff`, full `cargo test --workspace --all-targets`, strict OpenSpec validation and diff checks passed.
- Rebuilt the release binary and reran the fixed `c3318066` Python/Rust suite: 55/55 scenarios passed and the generated report was byte-identical to `provenance/PARITY_RUN.json`.

## 2026-09-22 — task 4.8 advanced domain values and v1 bidirectional projection

- Added independent `Keyframes`, `Mask`, `ChromaKey`, `BackgroundFilling`, `BlendMode`, `Animation`, `Transition`, `AudioEffects`/`Fade` and `TextStyle` value objects with segment-type validation. Their serde representation remains the existing flat Job v2 shape.
- Extended video, audio, text and sticker domain variants plus v1 input projection and Domain-to-Plan projection. The projection does not read the compatibility payload.
- Added a 55-fixture field-level v1→Domain→Plan contract covering crop/transform, keyframes, masks, chroma, backgrounds, blend modes, animations, transitions, video/audio fades, audio effects and complete text styling.
- Updated the checked-in Job v2 JSON Schema and parity-matrix gaps without promoting the four pyJianYingDraft rows beyond `partial`.
- TDD RED: the new domain contract initially failed to compile because the value objects and public Domain-to-Plan evidence projection did not exist.
- TDD GREEN: domain contracts 12/12, Job v2 contracts 8/8 and provenance contracts 4/4 passed; full workspace tests and Clippy with warnings denied passed.
- Fixed-source differential: the read-only upstream checkout resolved to `c3318066d964744e2bfc66f75c71745fe8cea52a`; Python/Rust draft parity passed 55/55 and the generated report was byte-identical to `provenance/PARITY_RUN.json` (SHA-256 `bc8c2a95e4c582274f35fd5e6f5e97635252f2e81264a1042603e0b2e56ef430`).

Task 4.8 is complete. Tasks 4.9–4.12 remain open: edit/reference-closure/final parity promotion are not claimed.

## 2026-09-22 — task 4.9 first production cutover increment

- Added a black-box conflict test that keeps the converted `DraftProject` unchanged while replacing the legacy compatibility name and text. Before the fix, the produced draft incorrectly followed the compatibility copy; after the fix, the draft follows only the domain project.
- Normal `jianying-cli-plan/v1` Job execution no longer deserializes or executes `compatibility.payload`. The response records `compiler=draft_project_to_wire` and the input compatibility schema without reading legacy payload fields.
- Added an optional track display name to the domain model and Job v2 schema. Stable track IDs remain unique while duplicate or absent v1 display names retain their original wire semantics.
- TDD RED reproduced compatibility authority drift. GREEN: the new black-box test, the v1/v2 observable equivalence test, all 13 Job CLI contracts, 8 Job v2 contracts, 12 domain contracts and Clippy with warnings denied pass.
- Compatibility-only `capcut-cli-compile/v1` now performs strict `CompileSpec` input conversion, projects its base timeline through `DraftProject`, and only then writes wire data. Its nine post-build actions consume the validated `CompileOperation` enum; both CLI and Job v2 black boxes report `compiler=draft_project_to_wire`.
- Release-mode fixed-source differential passed 55/55 and produced a byte-identical report to `provenance/PARITY_RUN.json`, SHA-256 `bc8c2a95e4c582274f35fd5e6f5e97635252f2e81264a1042603e0b2e56ef430`. Full workspace tests, Clippy with warnings denied, provenance gates, strict OpenSpec validation and diff checks pass.

Task 4.9 is complete. Tasks 4.10–4.12 remain open; edit closure, replacement/import reference semantics and final real-editor parity promotion are not claimed.

## 2026-09-22 — task 4.10 isolated typed edit closure

- Extended the closed `EditOperation` contract and checked-in Job v2 schema with stable-ID add/remove/reorder track operations.
- Implemented local video/audio/image declaration sequencing and full typed segment compilation through the existing `DraftProject -> Domain-to-Wire` compiler. The edit handler imports the generated primary and companion resource closure into the outer isolated draft, restores the declared segment ID and preserves the declared track ID.
- A single typed video black box covers crop/transform, keyframes, mask, chroma, background filling, blend mode, animation, transition and fade. Its final draft has no temporary track or segment, contains the required material buckets, owns its copied media and passes bundle verification.
- Added structured `invalid_job` failures for missing and unconsumed material declarations. Failure tests prove the source remains byte-identical and neither final output nor edit/domain-segment staging survives. Font and editor-only resources remain explicitly incompatible instead of being silently accepted.
- TDD RED: the new contract first failed because `add_track` was not a known operation. Subsequent black-box failures exposed relative Job-root identity handling, single-segment transition closure and orphan placeholder resources; each was fixed in the shared path.
- TDD GREEN: Job v2 contracts 9/9 and Job CLI contracts 15/15 pass. Full `cargo test --workspace --all-targets --locked`, Clippy with warnings denied, the provenance gate, strict OpenSpec validation and `git diff --check` pass.
- Release-mode fixed-source differential passed 55/55 against read-only pyJianYingDraft commit `c3318066d964744e2bfc66f75c71745fe8cea52a`; the generated report is byte-identical to `provenance/PARITY_RUN.json`, SHA-256 `bc8c2a95e4c582274f35fd5e6f5e97635252f2e81264a1042603e0b2e56ef430`.

Task 4.10 is complete. Tasks 4.11 and 4.12 remain open; replacement/import reference semantics and final real-editor promotion are not claimed.

## 2026-09-22 — task 4.11 replacement and import semantics

- Fixed eight pyJianYingDraft replacement-timing observations at read-only commit `c3318066d964744e2bfc66f75c71745fe8cea52a`: `cut_head`, `cut_tail`, `cut_tail_align`, `shrink`, `extend_head`, `extend_tail`, `push_tail` and ordered fallback to `cut_material_tail`.
- Added proportional text-style recalculation. ASCII behavior matches Python; Rust intentionally measures UTF-16 code units so emoji offsets remain valid, and the approved difference is recorded in the provenance artifact.
- `template replace-material` now exposes source range, crop replacement and ordered shrink/extend policies. Segment-scoped replacement creates a new material identity while other segments keep the shared original.
- Material replacement and track import now run through isolated `MutationPlan` commits. The failure black box proves the source timeline bytes and local file count remain unchanged.
- Track import recursively traverses material references in normal JSON fields and JSON-encoded strings, remaps the full material closure plus track/segment IDs, copies local assets into the target and leaves unrelated JSON strings untouched. Removing the source draft after import does not invalidate the target.
- TDD GREEN: template semantic differential 2/2, CLI replacement black box 3/3, template/import filesystem contracts 6/6 and provenance guard 4/4 passed. Full workspace tests and Clippy with warnings denied passed.
- Strict OpenSpec validation and the provenance gate with negative self-tests passed. Release-mode fixed-source draft parity remained 55/55 and byte-identical to `provenance/PARITY_RUN.json`, SHA-256 `bc8c2a95e4c582274f35fd5e6f5e97635252f2e81264a1042603e0b2e56ef430`.

Task 4.11 is complete. Task 4.12 remains open; the four partial pyJianYingDraft rows are not promoted without the required real-editor evidence.
# 2026-09-23：真实 Vlog 素材揭示代理帧率偏差与 Windows 锁竞争

三个已记录来源和 SHA-256 的 Pexels 旅行素材在独立验收目录中组成两轮
`jianying-job/v2`：六秒开场的 revision 1 为 16 秒，三秒开场的 revision 2 为
13 秒。不可变 CLI `v1.6.26` 均生成三片段、无结构问题的草稿和可解码代理，
但两份草稿声明 25 fps，发布代理却是 30 fps。故不能把旧代理作为帧率忠实的质量证据。

回归测试先以 `left: 30/1, right: 25/1` RED，再使 `render proxy` 从草稿读取并
校验 1–240 fps 的有限帧率，写入 FFmpeg 滤镜；25 和 30 fps 均由 ffprobe 独立
回读，0 fps 在输出文件创建前失败。对上述真实素材用本地 `v1.6.28` 候选重新
生成的两个代理，发布模式 CLI 独立探测为 25 fps，时长分别 16、13 秒。
这仍是代理证据，不是剪映冷重开、连续播放、原生导出或人工叙事验收。

`v1.6.27` tag 的发布流水线 `35788716642` 未发布：macOS arm64/x64 通过，
Windows 2025 的 256 任务/8 写入者 SQLite WAL 回归在 5 秒等待后返回
`database is locked`，publish 被跳过。候选将有限 busy timeout 提高到 30 秒，
保持 CAS 与 256 任务测试不变；本地全量工作区测试、Clippy 和严格 OpenSpec
通过。发布流水线 `35819162956` 的 source-parity、macOS arm64/x64、Windows 2025 与 publish 均成功；Windows 日志明确记录 `sqlite_rejects_lost_updates_for_one_task` 和 `json_and_sqlite_preserve_parallel_volume_without_record_loss` 均为 `ok`。后者保持 8 写入者/256 任务及无丢失断言。正式 `v1.6.28` Release 为非草稿、非预发布且 `isImmutable=true`，10 个资产齐全，`gh release verify` 与下载的 release index、darwin-arm64 归档 `verify-asset` 均通过。注释 tag 解引用至提交 `6e491133c9e8deec07373b66232f76f2f2bbb14b`，与发布源提交一致。

下载的正式 macOS arm64 二进制报告 `jianying 1.6.28`。它对真实旅行素材的 revision 2 草稿渲染 13 秒代理，JSON `target_fps=25`，独立 ffprobe 回读 `r_frame_rate=avg_frame_rate=25/1`、时长 `13.000000` 秒；只在隔离副本中把草稿 fps 改为 30 后，发布二进制输出 `30/1`；改为 0 后返回结构化失败 `proxy draft fps must be a finite number in [1, 240]`，目标文件未创建。下载索引 SHA-256 为 `9f03c770d936f35707e0f14a79889e79f6cbaabdb89f7be1da2d7829e7b76cd6`，归档 SHA-256 为 `0e9904a946ee7b1998e9e8fe7b83b8f3a2c25227b155cd618e6d68c4505e364b`。这完成 6.39、7.9 的发布制品回归，不等于真实剪映原生导出或人工视频质量验收。

## 2026-09-23 — 任务 6.40：片段缩放的发布代理黑盒回归

- 真实 Vlog 的 revision 3/4/5 代理曾逐字节相同，说明旧发布二进制未应用 `clip.scale`。以 180×320 红色视频、320×180 横屏画布和原生 `jianying-job/v2` 的 `scale=3.17` 建立 RED：旧渲染器抽取左边缘像素为 RGB `(0,0,0)`。修复后左边缘为红色；零缩放在输出文件创建前失败关闭。`cargo fmt`、Clippy、目标 Job/渲染测试及 OpenSpec strict 均通过。
- `main` 提交 `704d1ca9e0a6520ba36d3aac863de8d7e0f7c495` 的 CI `35828937710` 全绿；tag `v1.6.29` 的发布流水线 `35829653051` 完成来源差分、macOS arm64/x64、Windows x64 和 publish。正式 Release `isImmutable=true`，10 个资产；`gh release verify`、arm64 资产 `verify-asset` 和包内 `SHA256SUMS` 均通过。Release index SHA-256 为 `0acefa82b6915d2643c6dd72845b0244fd09d544b2db1043a79beae0b92f525b`，arm64 归档为 `0aad2768553fff9c69e5a0aff4d3f427fb1697a459ff2c66ce97954adb3ac086`，二进制为 `998ccdc4ec4e0aa00cd8bf3e52d132afd31a3332b8bdd76aafd09ab83c542d1e`。
- 下载的正式 arm64 二进制对同一 revision 5 草稿重新渲染：旧代理 SHA-256 `9642e10b073bb78c4bd4e57624d4163678e748d83983935dfc842416743dc01f`，新代理 `683c24f23131ad5d3f5062b3f332785c09c38bfdab9d2a2d6c3468d4e6f3a947`；第 5 秒左边缘独立抽帧从 RGB `(0,0,0)` 变为 `(107,168,237)`。ffprobe 回读 960×540、25 fps、视频及音频流，发布二进制 `project verify` 回报 13 秒、2 轨、无结构问题。此证据只完成代理片段缩放；`canvas_blur` 背景填充、剪映冷重开/播放/原生导出及人工叙事音画仍未验收。

## 2026-09-23 — 任务 6.41：逐片段 canvas_blur 的发布代理黑盒回归

- 真实 Vlog revision 4 的 `journey` 片段引用 `canvas_blur`，旧代理在第 5 秒画布左边缘仍是 RGB `(0,0,0)`。新增原生 Job v2 合同先以该失败为 RED，再覆盖相邻的有背景/无背景片段：前者的模糊侧边同时含红蓝源像素，后者保持黑边；悬空引用、无效强度和冲突的多个模糊引用均在输出前失败关闭。
- `render proxy` 现在只对引用有效 `canvas_blur` 的片段以同源画面生成近似模糊背景，再在透明前景之下合成；无引用片段保持旧构图。响应记录 `background_blur_segments` 并明确代理近似而非原生导出。目标渲染合同、Job v2 合同、完整工作区 Clippy、严格 OpenSpec 校验与 diff 检查通过。
- 提交 `db3a04a0a6becf35b1c4e29dabbd2a295126318a` 的 `main` CI `35836629326` 成功；正式 `v1.6.30` 发布流水线 `35837558424` 的来源差分、Windows x64、macOS arm64/x64 和 publish 全部成功。注释 tag 解引用及 Release index 均指向该提交。Release 非草稿、非预发布且 `isImmutable=true`，10 个资产齐全；`gh release verify`、下载的 arm64 归档 `verify-asset` 和包内 `SHA256SUMS` 均通过。Release index SHA-256 为 `6ec309d005ce518d29321b447bdfc0fd59ba86e20df630fe7cb513149cfda7f9`，arm64 归档为 `e56505aa0d250d36bed1810180ce0795c2b1b50c6460acd3cf01254bd8304b1c`，二进制为 `c08a2b2d467854b80f8ea95f5552be3508da9878ca97b8d1299313725dea1fe4`。
- 下载的正式 arm64 二进制报告 `jianying 1.6.30`，对未改动的 revision 4 草稿生成 `proxy-release-1.6.30.mp4`，SHA-256 `b5f22ec1e7556d267a7ee4e123196426666795c902fdbf162266b900bf11917c`。响应为 `background_blur_segments=1`、`target_fps=25`；ffprobe 独立回读 960×540、25 fps、约 13 秒及视频/音频流。第 5 秒左边缘独立抽帧从旧 RGB `(0,0,0)` 变为 `(113,169,229)`，截图可见前景竖屏航拍与两侧同源模糊背景。发布二进制 `project verify` 回报 13 秒、2 轨、无结构问题。
- 此证据只完成代理背景填充，不代表真实剪映冷重开、连续播放、原生导出或人工叙事/音画通过；这些门禁继续保持未完成。
