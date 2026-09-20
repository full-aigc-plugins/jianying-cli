#!/usr/bin/env python3
"""Semantic black-box differential for pinned capcut-cli utility commands."""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import tempfile


PINNED = "49f70e3b07f1a236d45acb9b49a70c141dd1ee98"


def invoke(argv: list[str]) -> str:
    process = subprocess.run(argv, text=True, capture_output=True, check=False)
    if process.returncode != 0:
        raise RuntimeError(
            f"command failed ({process.returncode}): {argv!r}\n"
            f"stdout={process.stdout}\nstderr={process.stderr}"
        )
    return process.stdout


def rust_json(binary: pathlib.Path, *args: str) -> dict:
    return json.loads(invoke([str(binary), *args, "--json"]))["data"]


def upstream_json(entry: pathlib.Path, *args: str) -> dict:
    return json.loads(invoke(["node", str(entry), *args, "--json"]))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--upstream", type=pathlib.Path, required=True)
    parser.add_argument("--rust-bin", type=pathlib.Path, default=pathlib.Path("target/debug/jianying"))
    parser.add_argument("--output", type=pathlib.Path)
    args = parser.parse_args()
    revision = subprocess.check_output(
        ["git", "-C", str(args.upstream), "rev-parse", "HEAD"], text=True
    ).strip()
    if revision != PINNED:
        raise RuntimeError(f"upstream revision {revision} does not match {PINNED}")
    entry = args.upstream / "dist/index.js"
    rust = args.rust_bin.resolve()
    if not entry.is_file() or not rust.is_file():
        raise RuntimeError("build both the pinned upstream and Rust CLI before running")

    cases = []
    shell_markers = {
        "bash": ["COMP_WORDS", "complete"],
        "zsh": ["#compdef", "_describe"],
        "fish": ["complete -c", "function"],
    }
    for shell, alternatives in shell_markers.items():
        upstream = invoke(["node", str(entry), "completions", shell])
        generated = invoke([str(rust), "completion", shell])
        assert upstream == invoke(["node", str(entry), "completions", shell])
        assert generated == invoke([str(rust), "completion", shell])
        assert "capcut" in upstream and "jianying" in generated
        assert any(marker in upstream for marker in alternatives)
        assert any(marker in generated for marker in alternatives)
        cases.append({
            "command": "completions",
            "shell": shell,
            "status": "semantic-command-map",
            "evidence": "deterministic non-empty shell-native script using the mapped executable name",
        })

    source_enums = json.loads((args.upstream / "src/enums.json").read_text(encoding="utf-8"))
    categories = {
        "transitions": "--transitions",
        "masks": "--masks",
        "image_intros": "--image-intros",
        "image_outros": "--image-outros",
        "image_combos": "--image-combos",
        "text_intros": "--text-intros",
        "text_outros": "--text-outros",
        "text_loop_anims": "--text-loop-anims",
        "scene_effects": "--scene-effects",
        "character_effects": "--character-effects",
        "audio_effects": "--audio-effects",
        "fonts": "--fonts",
        "filters": "--filters",
        "bubbles": "--bubbles",
    }
    for namespace in ["capcut", "jianying"]:
        for category, flag in categories.items():
            rust_category = category.replace("_", "-")
            generated = rust_json(
                rust, "media", "enums", rust_category, "--namespace", namespace
            )
            upstream_args = ["node", str(entry), "enums", flag]
            if namespace == "jianying":
                upstream_args.append("--jianying")
            upstream_raw = invoke(upstream_args)
            try:
                expected = json.loads(upstream_raw, strict=False)
                status = "exact"
                evidence = "fixed-upstream CLI entries and order"
            except json.JSONDecodeError:
                if (
                    not upstream_raw
                    or category == "bubbles"
                    or (namespace == "capcut" and category == "filters")
                    or category not in source_enums[namespace]
                ):
                    raise RuntimeError(
                        f"unexpected invalid upstream JSON: {namespace}/{category}, "
                        f"bytes={len(upstream_raw.encode())}"
                    )
                expected = source_enums[namespace].get(category, [])
                status = "approved-upstream-stdout-truncation"
                evidence = (
                    f"upstream exits zero after truncating piped JSON at "
                    f"{len(upstream_raw.encode())} bytes; "
                    "Rust entries and order equal the same pinned src/enums.json"
                )
            assert generated["namespace"] == namespace
            assert generated["category"] == category
            assert generated["count"] == len(expected)
            assert generated["entries"] == expected, (namespace, category, status)
            cases.append({
                "command": "enums",
                "namespace": namespace,
                "category": category,
                "status": status,
                "evidence": evidence,
            })

    with tempfile.TemporaryDirectory(prefix="jianying-harvest-diff-") as temporary:
        root = pathlib.Path(temporary)

        def write_draft(path: pathlib.Path, name: str, fresh: bool = False) -> None:
            path.mkdir(parents=True)
            filters = [
                {"name": "Known", "effect_id": "7028463716732079117", "resource_id": "7028463716732079117"},
                {"name": "Bubble", "type": "text_shape", "effect_id": "bubble-e", "resource_id": "bubble-r"},
            ]
            if fresh:
                filters.append({"name": "Fresh Filter", "effect_id": "fresh-e", "resource_id": "fresh-r"})
            timeline = {
                "id": name, "name": name, "tracks": [],
                "materials": {
                    "video_effects": [{"name": "Snow Fly", "effect_id": "custom-e", "resource_id": "custom-r"}],
                    "transitions": [], "audio_effects": [], "masks": [], "common_mask": [], "common_masks": [],
                    "filters": filters,
                    "material_animations": [{"animations": [{"id": "anim-e", "name": "Animation", "resource_id": "anim-r"}]}],
                    "texts": [{"font_id": "font-r"}],
                },
            }
            encoded = json.dumps(timeline, ensure_ascii=False, indent=2)
            (path / "draft_content.json").write_text(encoded, encoding="utf-8")
            (path / "draft_info.json").write_text(encoded, encoding="utf-8")

        draft_path = root / "draft"
        write_draft(draft_path, "Harvest")
        rust_catalogue = root / "rust-user-enums.json"
        upstream_catalogue = root / "upstream-user-enums.json"
        rust_scan = rust_json(
            rust, "media", "harvest-enums", "scan", str(draft_path), "--catalogue", str(rust_catalogue)
        )
        upstream_scan = json.loads(invoke([
            "node", str(entry), "harvest-enums", str(draft_path), "--catalogue", str(upstream_catalogue)
        ]))
        for key in ["applied", "found", "known", "new", "writable_slugs", "id_only"]:
            assert rust_scan[key] == upstream_scan[key]
        rust_scan_apply = rust_json(
            rust, "media", "harvest-enums", "scan", str(draft_path), "--catalogue", str(rust_catalogue), "--apply"
        )
        upstream_scan_apply = json.loads(invoke([
            "node", str(entry), "harvest-enums", str(draft_path), "--catalogue", str(upstream_catalogue), "--apply"
        ]))
        for key in ["applied", "added", "duplicates", "total", "found", "known", "new"]:
            assert rust_scan_apply[key] == upstream_scan_apply[key]
        rust_scan_entries = json.loads(rust_catalogue.read_text(encoding="utf-8"))["entries"]
        upstream_scan_entries = json.loads(upstream_catalogue.read_text(encoding="utf-8"))["entries"]
        assert [{k: v for k, v in item.items() if k != "harvested_at"} for item in rust_scan_entries] == [
            {k: v for k, v in item.items() if k != "harvested_at"} for item in upstream_scan_entries
        ]
        cases.append({"command": "harvest-enums", "mode": "scan", "status": "semantic-command-map",
                      "evidence": "fixed-upstream plan/apply, known-id, candidate, slug, id-only and persisted-entry semantics"})

        rust_add = rust_json(
            rust, "media", "harvest-enums", "add", "video_effects", "snow-fly", "manual-r",
            "--effect-id", "manual-e", "--catalogue", str(rust_catalogue)
        )
        upstream_add = json.loads(invoke([
            "node", str(entry), "harvest-enums", "--add", "video_effects", "snow-fly", "manual-r",
            "--effect-id", "manual-e", "--catalogue", str(upstream_catalogue)
        ]))
        assert rust_add["applied"] == upstream_add["applied"] == False
        assert rust_add["entry"] == upstream_add["entry"]
        rust_add_apply = rust_json(
            rust, "media", "harvest-enums", "add", "video_effects", "snow-fly", "manual-r",
            "--effect-id", "manual-e", "--catalogue", str(rust_catalogue), "--apply"
        )
        upstream_add_apply = json.loads(invoke([
            "node", str(entry), "harvest-enums", "--add", "video_effects", "snow-fly", "manual-r",
            "--effect-id", "manual-e", "--catalogue", str(upstream_catalogue), "--apply"
        ]))
        for key in ["applied", "added", "total", "entry"]:
            assert rust_add_apply[key] == upstream_add_apply[key], (key, rust_add_apply[key], upstream_add_apply[key])
        cases.append({"command": "harvest-enums", "mode": "add", "status": "semantic-command-map",
                      "evidence": "fixed-upstream manual validation plus plan/apply entry semantics"})

        library = root / "library"
        write_draft(library / "one", "One")
        write_draft(library / "two", "Two", fresh=True)
        (library / "broken").mkdir()
        (library / "broken" / "draft_content.json").write_text("not-json", encoding="utf-8")
        rust_sync = rust_json(
            rust, "media", "harvest-enums", "sync", "--drafts", str(library),
            "--catalogue", str(rust_catalogue)
        )
        upstream_sync = json.loads(invoke([
            "node", str(entry), "harvest-enums", "--sync", "--drafts", str(library),
            "--catalogue", str(upstream_catalogue)
        ]))
        for key in ["applied", "drafts_scanned", "found", "known", "new", "new_by_kind", "writable_slugs", "id_only"]:
            assert rust_sync[key] == upstream_sync[key], key
        assert len(rust_sync["drafts_skipped"]) == len(upstream_sync["drafts_skipped"]) == 1
        rust_sync_apply = rust_json(
            rust, "media", "harvest-enums", "sync", "--drafts", str(library),
            "--catalogue", str(rust_catalogue), "--apply"
        )
        upstream_sync_apply = json.loads(invoke([
            "node", str(entry), "harvest-enums", "--sync", "--drafts", str(library),
            "--catalogue", str(upstream_catalogue), "--apply"
        ]))
        for key in ["applied", "added", "duplicates", "total", "drafts_scanned", "found", "known", "new", "new_by_kind"]:
            assert rust_sync_apply[key] == upstream_sync_apply[key], key
        cases.append({"command": "harvest-enums", "mode": "sync", "status": "semantic-command-map",
                      "evidence": "fixed-upstream plan/apply sorted library sweep, cross-draft dedupe and broken-draft skip semantics"})

    with tempfile.TemporaryDirectory(prefix="jianying-diagnose-diff-") as temporary:
        root = pathlib.Path(temporary)
        draft_path = root / "draft"
        rust_json(rust, "project", "init", "diagnose", "--out", str(draft_path))

        def comparable(report: dict) -> dict:
            candidate_keys = [
                "file", "exists", "size", "sha256", "parseable_timeline", "envelope",
                "tracks", "segments", "app_version", "error",
            ]
            return {
                key: report.get(key)
                for key in ["ok", "project_dir", "canonical", "version", "modern_storage", "diverged", "layout", "nested_timelines"]
            } | {
                "candidates": [
                    {key: candidate.get(key) for key in candidate_keys if key in candidate}
                    for candidate in report["candidates"]
                ]
            }

        rust_report = rust_json(rust, "project", "diagnose", str(draft_path))
        upstream_report = json.loads(invoke(["node", str(entry), "diagnose", str(draft_path)]))
        assert comparable(rust_report) == comparable(upstream_report), (
            comparable(rust_report), comparable(upstream_report)
        )
        assert rust_report["candidates"][0]["timeline_hash"] == rust_report["candidates"][1]["timeline_hash"]
        assert upstream_report["candidates"][0]["timeline_hash"] == upstream_report["candidates"][1]["timeline_hash"]
        cases.append({"command": "diagnose", "mode": "canonical", "status": "semantic-command-map",
                      "evidence": "fixed-upstream canonical/layout/version, per-candidate byte hashes and timeline-equivalence relation"})

        info_path = draft_path / "draft_info.json"
        info = json.loads(info_path.read_text(encoding="utf-8"))
        info["name"] = "diverged"
        info_path.write_text(json.dumps(info, ensure_ascii=False, indent=2), encoding="utf-8")
        rust_bundle = root / "rust-diagnose.json"
        upstream_bundle = root / "upstream-diagnose.json"
        rust_diverged = rust_json(
            rust, "project", "diagnose", str(draft_path), "--bundle", str(rust_bundle)
        )
        upstream_diverged = json.loads(invoke([
            "node", str(entry), "diagnose", str(draft_path), "--bundle", str(upstream_bundle)
        ]))
        assert comparable(rust_diverged) == comparable(upstream_diverged)
        assert rust_diverged["diverged"] == upstream_diverged["diverged"] == True
        assert "bundle" not in json.loads(rust_bundle.read_text(encoding="utf-8"))
        assert "bundle" not in json.loads(upstream_bundle.read_text(encoding="utf-8"))
        assert any("diverge" in action.lower() for action in rust_diverged["next_actions"])
        assert any("diverge" in action.lower() for action in upstream_diverged["next_actions"])
        cases.append({"command": "diagnose", "mode": "divergence-bundle", "status": "semantic-command-map",
                      "evidence": "fixed-upstream sibling divergence plus redacted bundle semantics"})

    with tempfile.TemporaryDirectory(prefix="jianying-fixture-diff-") as temporary:
        root = pathlib.Path(temporary)
        draft_path = root / "home" / "secretuser" / "draft"
        draft_path.mkdir(parents=True)
        timeline = {
            "id": "fixture-draft",
            "name": "/home/secretuser/private.mov secret@example.com",
            "duration": 1_000_000,
            "fps": 30,
            "canvas_config": {"width": 1080, "height": 1920, "ratio": "9:16"},
            "platform": {
                "app_source": "cc", "app_version": "8.7.0", "os": "windows",
                "device_id": "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6",
                "mac_address": "0f1e2d3c4b5a69788796a5b4c3d2e1f0", "hard_disk_id": "",
            },
            "tracks": [],
            "materials": {"videos": [], "audios": [], "texts": [], "speeds": []},
        }
        encoded = json.dumps(timeline, ensure_ascii=False, indent=2)
        (draft_path / "draft_content.json").write_text(encoded, encoding="utf-8")
        (draft_path / "draft_info.json").write_text(encoded, encoding="utf-8")
        assets = draft_path / "assets" / "video"
        assets.mkdir(parents=True)
        (assets / "private.mp4").write_bytes(b"private-media")
        rust_bundle = root / "rust-bundle"
        upstream_bundle = root / "upstream-bundle"
        rust_report = rust_json(
            rust, "project", "fixture", str(draft_path), "--out", str(rust_bundle), "--check"
        )
        upstream_report = json.loads(invoke([
            "node", str(entry), "fixture", str(draft_path), "--out", str(upstream_bundle), "--check"
        ]))
        for key in ["ok", "version", "modern_storage", "media_excluded"]:
            assert rust_report[key] == upstream_report[key], key
        assert rust_report["redaction_check"]["ok"] == upstream_report["redaction_check"]["ok"] == True
        assert rust_report["mask_keyframe_evidence"] == upstream_report["mask_keyframe_evidence"]
        assert [item["file"] for item in rust_report["files"]] == [item["file"] for item in upstream_report["files"]]
        assert rust_report["redaction_kinds"] == upstream_report["redaction_kinds"]
        for bundle in [rust_bundle, upstream_bundle]:
            content = (bundle / "draft_content.json").read_text(encoding="utf-8")
            assert "secretuser" not in content and "a1b2c3d4" not in content
            assert "/home/USER/" in content and "redacted@example.com" in content
            assert not (bundle / "assets").exists()
            for required in ["README.md", "diagnose.json", "mask-keyframe-report.json", "SANITIZE_REPORT.json"]:
                assert (bundle / required).is_file(), (bundle, required)
        cases.append({"command": "fixture", "mode": "sanitize-check", "status": "semantic-command-map",
                      "evidence": "fixed-upstream timeline-only copy, path/email/device redaction, reports, media exclusion and clean mechanical check"})

        for bundle in [rust_bundle, upstream_bundle]:
            (bundle / "leak.json").write_text(
                json.dumps({"path": "/Users/hansmustermann/private.mov"}), encoding="utf-8"
            )
        rust_failed = subprocess.run(
            [str(rust), "project", "fixture", str(rust_bundle), "--check", "--json"],
            text=True, capture_output=True, check=False,
        )
        upstream_failed = subprocess.run(
            ["node", str(entry), "fixture", str(upstream_bundle), "--check"],
            text=True, capture_output=True, check=False,
        )
        assert rust_failed.returncode == upstream_failed.returncode == 1
        rust_failure = json.loads(rust_failed.stdout)
        upstream_failure = json.loads(upstream_failed.stdout)
        rust_finding = rust_failure["error"]["details"]["findings"][0]
        upstream_finding = upstream_failure["findings"][0]
        assert {key: rust_finding[key] for key in ["file", "line", "kind"]} == {
            key: upstream_finding[key] for key in ["file", "line", "kind"]
        }
        assert "hansmustermann" not in rust_failed.stderr
        assert "hansmustermann" not in upstream_failed.stderr
        cases.append({"command": "fixture", "mode": "verify-only-failure", "status": "semantic-command-map",
                      "evidence": "fixed-upstream nonzero exit, file/line/kind-only leak finding and no sensitive stderr echo"})

    with tempfile.TemporaryDirectory(prefix="jianying-compile-diff-") as temporary:
        root = pathlib.Path(temporary)
        for name in ["clip1.mp4", "clip2.mp4", "music.mp3"]:
            (root / name).write_bytes(b"synthetic")
        (root / "captions.srt").write_text(
            "1\n00:00:00,000 --> 00:00:01,000\nHello\n", encoding="utf-8"
        )
        template = {
            "name": "cta", "type": "text",
            "segment": {
                "id": "old-seg", "material_id": "old-mat",
                "target_timerange": {"start": 0, "duration": 1_000_000},
                "source_timerange": {"start": 0, "duration": 1_000_000},
                "extra_material_refs": [], "common_keyframes": [], "keyframe_refs": [],
                "speed": 1, "volume": 1, "visible": True, "reverse": False,
                "clip": {"alpha": 1, "rotation": 0, "scale": {"x": 1, "y": 1},
                         "transform": {"x": 0, "y": 0}},
            },
            "material": {"type": "texts", "data": {
                "id": "old-mat", "type": "text",
                "content": json.dumps({"styles": [{"range": [0, 3], "size": 15}], "text": "CTA"}),
            }},
            "extra_materials": [],
        }
        (root / "template.json").write_text(
            json.dumps(template, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        base_tracks = [
            {"type": "video", "items": [
                {"ref": "hero", "path": "clip1.mp4", "start": 0, "duration": 2,
                 "speed": 1.25, "opacity": 0.8, "scale": 1.1},
                {"path": "clip2.mp4", "start": 2, "duration": 3},
            ]},
            {"type": "audio", "items": [
                {"ref": "music", "path": "music.mp3", "start": 0, "duration": 5, "volume": 0.4}
            ]},
            {"type": "text", "items": [
                {"ref": "hook", "text": "Hook", "start": 0, "duration": 2,
                 "fontSize": 18, "color": "#FFD700", "y": -0.6}
            ]},
        ]
        rich_spec = {
            "name": "Compiled", "width": 720, "height": 1280, "fps": 30, "ratio": "9:16",
            "tracks": base_tracks,
            "operations": [
                {"op": "transition", "target": "hero", "slug": "dissolve", "duration": 0.4},
                {"op": "keyframe", "target": "hero", "property": "uniform_scale", "time": 0,
                 "value": 1, "easing": "ease-out"},
                {"op": "audio-fade", "target": "music", "fadeIn": 0.5, "fadeOut": 0.5},
                {"op": "text-style", "target": "hook",
                 "style": {"borderWidth": 0.08, "borderColor": "#000000"}},
                {"op": "text-ranges", "target": "hook",
                 "ranges": [{"start": 0, "end": 4, "font_color": "#00FF00"}]},
                {"op": "filter", "slug": "vintage", "start": 0, "duration": 2},
                {"op": "effect", "slug": "shake", "start": 0, "duration": 1},
                {"op": "template", "path": "template.json", "start": 2, "duration": 1,
                 "text": "Follow", "ref": "cta"},
                {"op": "captions", "path": "captions.srt", "trackName": "captions", "styleRef": "hook"},
            ],
        }
        spec_path = root / "spec.json"
        spec_path.write_text(json.dumps(rich_spec, ensure_ascii=False, indent=2), encoding="utf-8")

        rust_plan = rust_json(rust, "project", "compile", str(spec_path), "--check")
        upstream_plan = upstream_json(entry, "compile", str(spec_path), "--check")
        for key in ["ok", "name", "canvas", "tracks", "items", "operations", "refs"]:
            assert rust_plan[key] == upstream_plan[key], (key, rust_plan[key], upstream_plan[key])
        assert rust_plan["checked"] == upstream_plan["checked"] == True
        assert rust_plan["write"] == upstream_plan["write"] == False
        cases.append({"command": "compile", "mode": "preflight", "status": "semantic-command-map",
                      "evidence": "fixed-upstream full media/operation preflight, canvas, item, operation and ref summary"})

        rust_out = root / "rust-out"
        upstream_out = root / "upstream-out"
        rust_result = rust_json(rust, "project", "compile", str(spec_path), "--out", str(rust_out))
        upstream_result = upstream_json(entry, "compile", str(spec_path), "--out", str(upstream_out))
        for key in ["ok", "name", "tracks", "segments", "duration_us"]:
            assert rust_result[key] == upstream_result[key], (key, rust_result[key], upstream_result[key])
        assert set(rust_result["refs"]) == set(upstream_result["refs"]) == {"hero", "music", "hook", "cta"}

        def compile_signature(path: pathlib.Path) -> dict:
            timeline = json.loads((path / "draft_content.json").read_text(encoding="utf-8"))
            tracks = timeline["tracks"]
            materials = timeline["materials"]
            segments = [segment for track in tracks for segment in track.get("segments", [])]
            text_contents = []
            for material in materials.get("texts", []):
                try:
                    text_contents.append(json.loads(material.get("content", "{}")))
                except json.JSONDecodeError:
                    pass
            range_colors = [
                style.get("fill", {}).get("content", {}).get("solid", {}).get("color")
                for content in text_contents for style in content.get("styles", [])
            ]
            return {
                "canvas": timeline["canvas_config"],
                "duration": timeline["duration"],
                "has_transition": len(materials.get("transitions", [])) > 0,
                "has_audio_fade": len(materials.get("audio_fades", [])) > 0,
                "has_filter_track": any(track.get("type") == "filter" for track in tracks),
                "has_effect_track": any(track.get("type") == "effect" for track in tracks),
                "has_caption_track": any(track.get("name") == "captions" for track in tracks),
                "has_keyframe": any(segment.get("common_keyframes") for segment in segments),
                "has_template_text": any(content.get("text") == "Follow" for content in text_contents),
                "has_text_ranges": [0, 1, 0] in range_colors,
                "has_border": any(material.get("has_border") is True for material in materials.get("texts", [])),
            }

        rust_signature = compile_signature(rust_out)
        upstream_signature = compile_signature(upstream_out)
        assert rust_signature == upstream_signature, (rust_signature, upstream_signature)
        assert all(value is True for key, value in rust_signature.items() if key.startswith("has_"))
        cases.append({"command": "compile", "mode": "nine-operations-build", "status": "semantic-command-map",
                      "evidence": "fixed-upstream refs, counts, duration and observable transition/keyframe/fade/style/ranges/filter/effect/template/captions semantics"})

        batch_spec = dict(rich_spec)
        batch_spec["name"] = "{{name}}"
        batch_spec["operations"] = []
        batch_spec["tracks"] = json.loads(json.dumps(base_tracks))
        batch_spec["tracks"][2]["items"][0]["text"] = "{{title}} for {{price}}"
        batch_path = root / "batch.json"
        batch_path.write_text(json.dumps(batch_spec, ensure_ascii=False, indent=2), encoding="utf-8")
        rows = root / "rows.jsonl"
        rows.write_text(
            '{"name":"A","title":"Alpha","price":9}\n'
            '{"name":"B","title":"Beta","price":19}\n', encoding="utf-8"
        )
        rust_store = root / "rust-store"
        upstream_store = root / "upstream-store"
        rust_store.mkdir()
        upstream_store.mkdir()
        rust_batch = rust_json(
            rust, "project", "compile", str(batch_path), "--data", str(rows), "--drafts", str(rust_store)
        )
        upstream_batch = upstream_json(
            entry, "compile", str(batch_path), "--data", str(rows), "--drafts", str(upstream_store)
        )
        assert [(row["row"], row["ok"], row["name"]) for row in rust_batch] == [
            (row["row"], row["ok"], row["name"]) for row in upstream_batch
        ]
        assert all((store / name / "draft_content.json").is_file()
                   for store in [rust_store, upstream_store] for name in ["A", "B"])
        cases.append({"command": "compile", "mode": "jsonl-batch", "status": "semantic-command-map",
                      "evidence": "fixed-upstream placeholder substitution, row ordering, distinct draft naming and two-draft build"})

        bad_rows = root / "bad-rows.jsonl"
        bad_rows.write_text('{"name":"valid","title":"A","price":1}\n{"wrong":"missing"}\n', encoding="utf-8")
        rust_fail_store = root / "rust-fail-store"
        upstream_fail_store = root / "upstream-fail-store"
        rust_fail_store.mkdir()
        upstream_fail_store.mkdir()
        rust_failed = subprocess.run(
            [str(rust), "project", "compile", str(batch_path), "--data", str(bad_rows),
             "--drafts", str(rust_fail_store), "--json"], text=True, capture_output=True, check=False,
        )
        upstream_failed = subprocess.run(
            ["node", str(entry), "compile", str(batch_path), "--data", str(bad_rows),
             "--drafts", str(upstream_fail_store), "--json"], text=True, capture_output=True, check=False,
        )
        assert rust_failed.returncode == upstream_failed.returncode == 1
        assert list(rust_fail_store.iterdir()) == [] and list(upstream_fail_store.iterdir()) == []
        assert "no drafts written" in rust_failed.stdout and "no drafts written" in upstream_failed.stderr
        cases.append({"command": "compile", "mode": "batch-fail-fast", "status": "semantic-command-map",
                      "evidence": "fixed-upstream 1-based failing row, nonzero exit and all-row preflight with zero draft writes"})

    report = {
        "schema": "jianying-capcut-utility-differential/v1",
        "upstream": {"repository": "renezander030/capcut-cli", "commit": revision},
        "runner": "tools/capcut_utility_differential.py",
        "cases": cases,
        "passed": len(cases),
        "failed": 0,
    }
    encoded = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
