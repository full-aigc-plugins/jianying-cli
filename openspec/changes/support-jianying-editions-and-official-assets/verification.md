# Verification

## 2026-10-21 — official resource contract: every refusal path (4.2, partial)

- Built synthetic entitlement and receipt fixtures from `schemas/jianying-entitlement-v1` and
  `schemas/jianying-official-resource-receipt-v1`, labelled synthetic in their own `title`, and drove
  `jianying media official verify … --at 1760000100` through eight cases: valid (verified), expired,
  basic edition against a professional asset, inactive, usage not allowed, tampered content, a receipt
  naming one file while another is passed, and `preview_only`. Seven refuse, one verifies.
- First run showed "sha256 mismatch → ok", which looked like a hole. The fixture was wrong: the file
  passed was the one the receipt described. Re-probing with the receipt and the passed file
  disagreeing fires the identity check in both directions.
- `.gitattributes` marks `*.dat` binary and the two resource fixtures were rewritten with intended
  bytes, because the receipts hash those bytes; `core.autocrlf` would otherwise break the fixture on
  checkout (`7ef2f53`).
- Evidence: `docs/runbooks/evidence/official-resource/REFUSAL_MATRIX_20261021.json` plus nine fixtures.
- Not claimed: no real asset was downloaded; price files, government authorisation and user-visible
  notices have no field in the receipt contract, so 4.2 stays open.

## 2026-10-21 — basic-edition downgrade acceptance (4.4, complete)

- `tools/official_downgrade_acceptance.py` drives the release binary twice and asserts five things: a
  local-material draft still builds under a basic entitlement snapshot; the professional receipt is
  refused with no ready result; the refusal names the required edition; `media official preflight`
  refuses the same requirement; and the draft tree is byte-identical before and after, so nothing was
  substituted. Both runs agree, which is what makes it reproducible.
- Two defects in the driver itself were fixed: the report now relativises the throw-away work directory,
  and it compares the draft's file set rather than its digests, because `build` mints fresh ids.
- Evidence: `docs/runbooks/evidence/official-resource/DOWNGRADE_ACCEPTANCE_20261021.json`, commit `46aac32`.
- Not claimed: the downgraded environment is an entitlement snapshot fed to the CLI, not a real
  basic-edition account, and the editor was not driven.

## 2026-10-21 — UI enumeration on 11.6.0.14552 (2.12, partial)

- Registered the main menu (9 items), the left tab column (13), the media-type toolbar (11), the text
  panel (9 groups), the timeline toolbar (icons only), the export settings (6 fields), the preview bench
  (timecode, duration, play, fit, fullscreen, 9:16, zoom) and the publish restrictions.
- Publish restrictions are the substantive find: the 官方素材 panel labels each material with
  `可商用` or `不可商用` in the preview bar and shows a copyright banner, which is exactly the
  user-visible restriction the task asks to register and it maps onto a receipt's `allowed_usages`.
- Evidence: `docs/runbooks/evidence/windows-canary/UI_CONTROLS_11.6.0.14552.json` with crops `47`, `48`.
- Not enumerated: the segment context menu, after five attempts; `46-segment-right-click.png` records
  the attempt where another application's always-on-top window covered the editor.

## 2026-10-21 — editor drafts are encrypted and are rewritten on save

- 剪映 encrypts the `draft_content.json` of the drafts it creates itself: `10月21日` is a Base64 envelope
  (3832 bytes → 2873 high-entropy bytes; no single-byte XOR key recovers it), while every CLI-built draft
  stays plain JSON.
- An editor save rewrote a 5397-byte CLI draft into 1767 bytes: `fps`, `source`, `extra_info.created_via`
  and twelve non-empty `config` keys dropped, empty buckets pruned, and draft, material and segment ids
  regenerated. Canvas, duration, text content and timing survive.
- Both facts changed the product rather than the prose: `DraftContentEncrypted` (`9beb6b8`) names the
  encrypted envelope instead of leaking a serde message, and `DraftContentChanged` (`73a3b39`) names the
  drift with both digests when a receipt no longer matches because the editor regenerated the ids.
- Evidence: `docs/runbooks/evidence/windows-canary/EDITOR_SAVE_NORMALIZATION_20261021.json` and
  `docs/runbooks/evidence/official-resource/DRAFT_DRIFT_20261021.json`.

## 2026-10-21 — final gates (4.5, partial)

- `openspec validate --all --strict` 2 passed / 0 failed; `cargo fmt --check`, `cargo clippy --all-targets
  -- -D warnings`, `cargo test --workspace` (123 targets, 383 passed) and `python -m unittest` (40) all
  clean; `check_provenance` including the negative self-test passes.
- `parity_run.py` reaches 55/55 against the pinned upstream `6bb2d06`, and the regenerated
  `provenance/PARITY_RUN.json` is byte-identical to the committed one. Ten frozen artifacts are recorded
  with their sha256 and are all LF-only.
- Reproducing it needs ffmpeg on PATH, `tools/` on `PYTHONPATH` for the pymediainfo adapter, and
  `--pyjyd-src` pointing at the package directory because `parity_reference.py` takes its parent.
- Evidence: `docs/runbooks/evidence/release/FINAL_GATES_20261021.json`, commit `0ec1050`.

## Open items and why they are open

| Task | Blocker |
| --- | --- |
| 2.12 | the publish-settings surface has not been opened; the context menu resisted five capture attempts |
| 4.1 | the material's licence terms are not obtainable: the app shows only a 可商用/不可商用 label and a banner |
| 4.2, 4.3 | a receipt needs an official material inside a draft the editor does not own; the editor encrypts its own drafts and rewrites CLI ones on save |
| 4.5 | publishability depends on the four above, and tagging or publishing needs the owner's explicit approval |

No task in this change is marked done on the strength of a simulation, a synthetic fixture or a
document that says it works.
