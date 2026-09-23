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
