#!/usr/bin/env python3
"""Dual-source parity harness: the SAME plan is built by pyJianYingDraft
(reference implementation) and by jianying-cli; the two drafts are compared
semantically. pyJianYingDraft output is the authority for wire shapes.

Usage:
    python3 tools/parity_run.py [--bin target/release/jianying] [--pyjyd-src DIR]
                                [--scenario tests/parity/scenarios/xx.json]

Each scenario file: {name, pyjyd_allowed_extra?: bool, plan: {...}}.
Volatile fields (ids, timestamps, absolute paths) are excluded; entries are
matched semantically. pyJYD-extras are allowed (capcut-compat fields the CLI
adds on top are reported as notes, not failures).
"""
import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

VOLATILE_KEYS = {"id", "material_id", "raw_segment_id", "local_material_id",
                 "music_id", "draft_id", "create_time", "update_time",
                 "tm_draft_create", "tm_draft_modified", "import_time",
                 "import_time_ms", "md5"}
PATH_KEYS = {"path", "file_Path", "media_path", "draft_json_file",
             "draft_fold_path", "draft_root_path", "draft_cover"}


def canon(v, drop_ids=True):
    """Canonicalize a draft into a semantic tree; ids vanish, paths vanish,
    numbers normalize to float (8 == 8.0)."""
    if isinstance(v, bool):
        return v
    if isinstance(v, (int, float)):
        return float(v)
    if isinstance(v, dict):
        out = {}
        for k, val in sorted(v.items()):
            if drop_ids and k in VOLATILE_KEYS:
                continue
            if k in PATH_KEYS:
                name = Path(str(val)).name if val else ""
                out[k] = f"<name:{name}>" if name else "<none>"
                continue
            out[k] = canon(val, drop_ids)
        return out
    if isinstance(v, list):
        return [canon(x, drop_ids) for x in v]
    return v


def segment_semantics(tl):
    """Extract per-track segment semantics, resolving refs to entries."""
    mats = tl.get("materials", {})

    def find_entry(ref):
        for bucket, items in sorted(mats.items()):
            if not isinstance(items, list):
                continue
            for m in items:
                if m.get("id") == ref:
                    return (bucket,) + tuple(sorted(canon(m, drop_ids=True).items()))
        return None  # pyJYD leaves dangling refs (e.g. text border id) — ignorable

    out = []
    for t in tl.get("tracks", []):
        segs = []
        for s in t.get("segments", []):
            entry = {}
            for k in ("target_timerange", "source_timerange", "speed", "volume",
                      "clip", "uniform_scale", "render_index"):
                if k in s and s[k] is not None:
                    entry[k] = canon(s[k], drop_ids=True)
            kfs = s.get("common_keyframes") or []
            if kfs:
                entry["keyframes"] = sorted(
                    (kf.get("property_type"),
                     [p.get("values") for p in kf.get("keyframe_list", [])])
                    for kf in kfs)
            refs = tuple(sorted(
                e for e in (find_entry(r) for r in s.get("extra_material_refs", [])
                            if r != s.get("material_id"))
                if e is not None))
            if refs:
                entry["refs"] = refs
            entry["material_bucket"] = material_bucket(mats, s.get("material_id", ""))
            segs.append(entry)
        out.append({"type": t.get("type"), "name": t.get("name"), "segments": segs})
    return out


def material_bucket(mats, material_id):
    for bucket, items in sorted(mats.items()):
        if isinstance(items, list):
            for m in items:
                if m.get("id") == material_id:
                    return bucket
    return "?"


def material_semantics(tl):
    """Returns {bucket: [entry_dict, ...]} with ids dropped."""
    """Semantic multiset of material entries that carry content."""
    keep_keys = ["type", "name", "material_name", "resource_id", "effect_id",
                 "value", "duration_us", "fade_in_duration", "fade_out_duration",
                 "config", "color", "intensity_value", "shadow_value",
                 "edge_smooth_value", "spill_value", "should_transfer_color",
                 "content", "is_overlap", "apply_target_type", "version",
                 "sticker_id", "width", "height", "check_flag", "text_color",
                 "alignment", "font_size", "line_spacing", "background_color",
                 "background_style", "border_width", "border_color"]
    out = {}
    for bucket, items in tl.get("materials", {}).items():
        if not isinstance(items, list):
            continue
        entries = []
        for m in items:
            slim = {}
            for k in keep_keys:
                if k not in m:
                    continue
                if k == "content" and isinstance(m[k], str):
                    try:
                        slim[k] = canon(json.loads(m[k]))
                    except Exception:
                        slim[k] = m[k]
                else:
                    slim[k] = canon(m[k], drop_ids=True)
            if slim:
                entries.append(slim)
        out[bucket] = entries
    return out


def entry_subset(ref_entry, cli_entries):
    """A reference entry is satisfied when some CLI entry carries every
    reference key with an equal value (CLI may carry capcut-compat extras)."""
    for c in cli_entries:
        if all(c.get(k) == v for k, v in ref_entry.items()):
            return True
    return False


def compare(ref_tl, cli_tl, superset=False):
    issues = []
    ref_tracks = segment_semantics(ref_tl)
    cli_tracks = segment_semantics(cli_tl)
    if len(ref_tracks) != len(cli_tracks):
        issues.append(f"track count {len(cli_tracks)} != reference {len(ref_tracks)}")
    for i, (r, c) in enumerate(zip(ref_tracks, cli_tracks)):
        if r["type"] != c["type"]:
            issues.append(f"track {i} type {c['type']} != reference {r['type']}")
        for j, (rs, cs) in enumerate(zip(r["segments"], c["segments"])):
            for k in rs:
                if k == "material_bucket" or k == "refs":
                    continue
                if cs.get(k) != rs[k]:
                    issues.append(f"track {i} seg {j} field {k}: "
                                  f"{json.dumps(cs.get(k), ensure_ascii=False)} != "
                                  f"reference {json.dumps(rs[k], ensure_ascii=False)}")
            for k in rs.get("refs", ()):
                if k not in cs.get("refs", ()):
                    issues.append(f"track {i} seg {j}: missing ref entry {json.dumps(k, ensure_ascii=False)[:200]}")
    if superset:
        # CLI extensions (e.g. multi-range styled text) are supersets of the
        # reference: compare only the base style
        for m in cli_tl.get("materials", {}).get("texts", []):
            if isinstance(m.get("content"), str):
                try:
                    c = json.loads(m["content"])
                    if isinstance(c.get("styles"), list) and len(c["styles"]) > 1:
                        c["styles"] = [c["styles"][0]]
                        m["content"] = json.dumps(c, ensure_ascii=False)
                except Exception:
                    pass
    ref_mats = material_semantics(ref_tl)
    cli_mats = material_semantics(cli_tl)
    for bucket, ref_entries in ref_mats.items():
        cli_entries = cli_mats.get(bucket, [])
        for re_ in ref_entries:
            if not entry_subset(re_, cli_entries):
                issues.append(
                    f"missing {bucket} entry: {json.dumps(re_, ensure_ascii=False)[:400]}")
    return issues


def run(cmd):
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(f"command failed: {' '.join(map(str, cmd))}\n{r.stdout}\n{r.stderr}")
    return r.stdout


def build_reference(plan_path, out_dir, pyjyd_src):
    script = Path(tempfile.mkstemp(suffix=".py")[1])
    script.write_text(f'''
import sys, os, json
sys.path.insert(0, {json.dumps(str(pyjyd_src))})
from pyJianYingDraft import parity_reference
parity_reference.build({json.dumps(str(plan_path))}, {json.dumps(str(out_dir))})
''')
    run([sys.executable, str(script)])


def main():
    args = sys.argv[1:]
    bin_path = ROOT / "target" / "release" / "jianying"
    pyjyd_src = Path("/tmp/pyjyd-study/pyJianYingDraft")
    scenario_filter = None
    i = 0
    while i < len(args):
        if args[i] == "--bin":
            bin_path = Path(args[i + 1]); i += 2
        elif args[i] == "--pyjyd-src":
            pyjyd_src = Path(args[i + 1]); i += 2
        elif args[i] == "--scenario":
            scenario_filter = args[i + 1]; i += 2
        else:
            i += 1

    scenarios = sorted((Path(__file__).parent.parent / "tests" / "parity" / "scenarios").glob("*.json"))
    if scenario_filter:
        scenarios = [s for s in scenarios if scenario_filter in s.name]

    passed, failed = 0, []
    for sc in scenarios:
        spec = json.loads(sc.read_text())
        name = spec["name"]
        with tempfile.TemporaryDirectory(prefix=f"parity-{name}-") as tmp:
            tmp = Path(tmp)
            plan_path = tmp / "plan.json"
            plan_path.write_text(json.dumps(spec["plan"], ensure_ascii=False))
            media = spec.get("media", [])
            for m in media:
                run(["ffmpeg", "-v", "error", "-f", "lavfi", "-i", m["gen"], "-y", str(tmp / m["name"])])
            try:
                run([sys.executable, str(ROOT / "tools" / "parity_reference.py"),
                     str(plan_path), str(tmp / "ref"), str(pyjyd_src)])
                run([str(bin_path), "build", str(plan_path), "--out", str(tmp / "cli")])
                ref_tl = json.loads(next((tmp / "ref" / "ref").glob("*/draft_content.json")).read_text())
                cli_tl = json.loads((tmp / "cli" / "draft_content.json").read_text())
                issues = compare(ref_tl, cli_tl, superset=bool(spec.get("expect_superset")))
            except RuntimeError as exc:
                issues = [f"harness error: {exc}"]
        if issues:
            failed.append((name, issues))
            print(f"FAIL {name}")
            for msg in issues[:6]:
                print(f"     - {msg}")
        else:
            passed += 1
            print(f"PASS {name}")
    print(f"\nparity: {passed} passed, {len(failed)} failed")
    if failed:
        sys.exit(1)


if __name__ == "__main__":
    main()
