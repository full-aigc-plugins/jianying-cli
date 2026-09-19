# Asset provenance

- `draft_content_template.json`, `draft_meta_info.json`: copied from
  [GuanYixuan/pyJianYingDraft](https://github.com/GuanYixuan/pyJianYingDraft)
  `pyJianYingDraft/assets/` (Apache-2.0). Minimal skeleton that 剪映 professional
  recognizes; stamps (id/name/canvas/fps/platform/times) are applied by the CLI.
- `transitions.json`: curated subset (non-VIP common transitions) extracted from
  the same project's `pyJianYingDraft/metadata/transition_meta.py` (Apache-2.0).
  Resource id/effect_id/md5 tuples are JianYing interoperability data.

Architecture reference (format facts only, no code copied):
- [renezander030/capcut-cli](https://github.com/renezander030/capcut-cli) (MIT):
  multi-mirror write discipline (draft_content.json + draft_info.json),
  draft_meta_info entry shape, root_meta_info.json registration, editor-running
  guard, render_index conventions, text content JSON-in-string shape.
- [partme-ai/jianying-headless](https://github.com/partme-ai/jianying-headless)
  (Personal Learning and Non-Commercial): plan-contract/workflow facts only
  (build → verify → publish gates); NO code or assets taken.
