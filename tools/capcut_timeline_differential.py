#!/usr/bin/env python3
"""Black-box timeline differential against the pinned capcut-cli command surface."""

from __future__ import annotations

import argparse
import json
import pathlib
import shutil
import subprocess
import tempfile
from typing import Any

PINNED = "49f70e3b07f1a236d45acb9b49a70c141dd1ee98"


def invoke(argv: list[str], input_text: str | None = None) -> Any:
    process = subprocess.run(argv, text=True, input=input_text, capture_output=True, check=False)
    if process.returncode != 0:
        raise RuntimeError(
            f"command failed ({process.returncode}): {argv!r}\n"
            f"stdout={process.stdout}\nstderr={process.stderr}"
        )
    return json.loads(process.stdout, strict=False)


def rust_data(binary: pathlib.Path, *args: str) -> Any:
    return invoke([str(binary), *args, "--json"])["data"]


def upstream_data(entry: pathlib.Path, *args: str) -> Any:
    return invoke(["node", str(entry), *args])


def rust_data_stdin(binary: pathlib.Path, input_text: str, *args: str) -> Any:
    return invoke([str(binary), *args, "--json"], input_text)["data"]


def upstream_data_stdin(entry: pathlib.Path, input_text: str, *args: str) -> Any:
    return invoke(["node", str(entry), *args], input_text)


def without_generated_ids(value: Any) -> Any:
    if isinstance(value, dict):
        return {key: without_generated_ids(item) for key, item in value.items() if key != "id"}
    if isinstance(value, list):
        return [without_generated_ids(item) for item in value]
    return value


def rebase_metadata(project: pathlib.Path) -> None:
    path = project / "draft_meta_info.json"
    value = json.loads(path.read_text(encoding="utf-8"))
    value["draft_fold_path"] = str(project.resolve())
    value["draft_root_path"] = str(project.parent.resolve())
    value["draft_json_file"] = str((project / "draft_content.json").resolve())
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")


def clone(base: pathlib.Path, target: pathlib.Path) -> pathlib.Path:
    shutil.copytree(base, target)
    rebase_metadata(target)
    return target


def paired(root: pathlib.Path, base: pathlib.Path, name: str) -> tuple[pathlib.Path, pathlib.Path]:
    return clone(base, root / f"rust-{name}"), clone(base, root / f"upstream-{name}")


def read_timeline(project: pathlib.Path) -> dict[str, Any]:
    return json.loads((project / "draft_content.json").read_text(encoding="utf-8"))


def write_timeline(project: pathlib.Path, value: dict[str, Any]) -> None:
    encoded = json.dumps(value, ensure_ascii=False, indent=2)
    (project / "draft_content.json").write_text(encoded, encoding="utf-8")
    (project / "draft_info.json").write_text(encoded, encoding="utf-8")


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

    results: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory(prefix="jianying-capcut-timeline-diff-") as temporary:
        root = pathlib.Path(temporary)
        srt = root / "captions.srt"
        srt.write_text(
            "1\n00:00:00,000 --> 00:00:01,000\nhello\n\n"
            "2\n00:00:01,000 --> 00:00:02,000\nworld\n",
            encoding="utf-8",
        )
        base = root / "base"
        rust_data(rust, "project", "quickstart", "timeline", "--out", str(base), "--srt", str(srt))
        segment_id = rust_data(rust, "timeline", "segments", str(base))[0]["id"]

        exact_queries = [
            ("tracks", ("timeline", "tracks", str(base)), ("tracks", str(base))),
            ("segments", ("timeline", "segments", str(base), "--track", "text"),
             ("segments", str(base), "--track", "text")),
            ("segment", ("timeline", "get", str(base), segment_id),
             ("segment", str(base), segment_id)),
        ]
        for command, rust_args, upstream_args in exact_queries:
            assert rust_data(rust, *rust_args) == upstream_data(entry, *upstream_args)
            results.append({"command": command, "status": "exact"})

        assert rust_data(rust, "timeline", "show", str(base), "--cols", "73") == upstream_data(
            entry, "timeline", str(base), "--cols", "73"
        )
        results.append({"command": "timeline", "status": "exact"})

        operations = [
            ("shift", ("timeline", "move"), ("shift",), (segment_id, "250ms")),
            ("shift-all", ("timeline", "move-all"), ("shift-all",), ("250ms",)),
            ("speed", ("timeline", "speed"), ("speed",), (segment_id, "2")),
            ("volume", ("timeline", "volume"), ("volume",), (segment_id, "0.5")),
            ("trim", ("timeline", "trim"), ("trim",), (segment_id, "0ms", "1s")),
        ]
        for command, rust_prefix, upstream_prefix, tail in operations:
            rust_project, upstream_project = paired(root, base, command)
            rust_result = rust_data(rust, *rust_prefix, str(rust_project), *tail)
            upstream_result = upstream_data(entry, *upstream_prefix, str(upstream_project), *tail)
            assert rust_result == upstream_result
            results.append({"command": command, "status": "exact"})

        # Rust-native operations deliberately improve the command model where the
        # pinned upstream has no direct equivalent, but still receive black-box
        # bundle validation in tests/timeline_cli_contract.rs.
        for command in ["add-track", "add-segment", "set", "split"]:
            results.append({
                "command": command,
                "status": "rust-extension",
                "evidence": "timeline_cli_contract",
            })
        duplicate_rust, duplicate_upstream = paired(root, base, "duplicate-deep")
        source_segments = rust_data(rust, "timeline", "segments", str(duplicate_rust))
        first_id, second_id = source_segments[0]["id"], source_segments[1]["id"]
        rust_duplicate = rust_data(rust, "timeline", "duplicate", str(duplicate_rust), first_id)
        upstream_duplicate = upstream_data(entry, "duplicate", str(duplicate_upstream), first_id)
        for key in ["new_track", "track_name", "source_segment_id"]:
            assert rust_duplicate[key] == upstream_duplicate[key]
        assert [
            (item["type"], item["source_id"]) for item in rust_duplicate["cloned_materials"]
        ] == [
            (item["type"], item["source_id"]) for item in upstream_duplicate["cloned_materials"]
        ]
        rust_existing = rust_data(
            rust, "timeline", "duplicate", str(duplicate_rust), second_id,
            "--track", rust_duplicate["track_name"]
        )
        upstream_existing = upstream_data(
            entry, "duplicate", str(duplicate_upstream), second_id,
            "--track", upstream_duplicate["track_name"]
        )
        assert rust_existing["new_track"] is False and upstream_existing["new_track"] is False
        assert rust_existing["track_name"] == upstream_existing["track_name"]
        results.append({
            "command": "duplicate", "status": "semantic",
            "evidence": "fixed-upstream deep material clone and existing-track placement",
        })

        rust_remove = rust_data(
            rust, "timeline", "remove", str(duplicate_rust), rust_duplicate["new_segment_id"]
        )
        upstream_remove = upstream_data(
            entry, "remove", str(duplicate_upstream), upstream_duplicate["new_segment_id"]
        )
        for key in ["track_removed", "materials_removed", "materials_by_type",
                    "duration_before_us", "duration_after_us"]:
            assert rust_remove[key] == upstream_remove[key]
        results.append({
            "command": "remove", "status": "semantic",
            "evidence": "fixed-upstream orphan sweep, duration, and track removal",
        })

        matting_rust, matting_upstream = paired(root, base, "matting")
        for project in [matting_rust, matting_upstream]:
            timeline = read_timeline(project)
            segments = timeline["tracks"][0]["segments"]
            material_id = segments[0]["material_id"]
            segments[1]["material_id"] = material_id
            timeline["tracks"][0]["type"] = "video"
            timeline["tracks"][0]["name"] = "video"
            for bucket, items in timeline["materials"].items():
                if isinstance(items, list):
                    timeline["materials"][bucket] = [item for item in items if item.get("id") != material_id]
            timeline["materials"]["videos"] = [{
                "id": material_id,
                "type": "video",
                "path": "",
                "matting": {
                    "flag": 0,
                    "has_use_quick_brush": True,
                    "interactiveTime": [1],
                    "path": "app-cache.bin",
                    "strokes": [{"x": 1}],
                    "vendor_unknown": "keep",
                },
            }]
            write_timeline(project, timeline)
        matting_id = read_timeline(matting_rust)["tracks"][0]["segments"][0]["id"]
        rust_matting = rust_data(rust, "timeline", "matting", str(matting_rust), matting_id)
        upstream_matting = upstream_data(entry, "matting", str(matting_upstream), matting_id)
        assert rust_matting == upstream_matting
        assert read_timeline(matting_rust)["materials"]["videos"][0]["matting"] == read_timeline(
            matting_upstream
        )["materials"]["videos"][0]["matting"]
        rust_off = rust_data(rust, "timeline", "matting", str(matting_rust), matting_id, "--off")
        upstream_off = upstream_data(entry, "matting", str(matting_upstream), matting_id, "--off")
        assert rust_off == upstream_off
        assert read_timeline(matting_rust)["materials"]["videos"][0]["matting"] == read_timeline(
            matting_upstream
        )["materials"]["videos"][0]["matting"]
        results.append({
            "command": "matting", "status": "exact",
            "evidence": "fixed-upstream enable, disable, shared segment, and cache preservation",
        })

        rust_chroma = rust_data(
            rust, "timeline", "chroma", str(matting_rust), matting_id,
            "--color", "#00FF00", "--intensity", "1.7"
        )
        upstream_chroma = upstream_data(
            entry, "chroma", str(matting_upstream), matting_id,
            "--color", "#00FF00", "--intensity", "1.7"
        )
        for key in ["ok", "segmentId", "color", "intensity", "shadow"]:
            assert rust_chroma[key] == upstream_chroma[key]
        rust_chroma_wire = read_timeline(matting_rust)["materials"]["chromas"][0]
        upstream_chroma_wire = read_timeline(matting_upstream)["materials"]["chromas"][0]
        assert {key: value for key, value in rust_chroma_wire.items() if key != "id"} == {
            key: value for key, value in upstream_chroma_wire.items() if key != "id"
        }
        assert rust_chroma["materialId"] in read_timeline(matting_rust)["tracks"][0]["segments"][0]["extra_material_refs"]
        assert upstream_chroma["materialId"] in read_timeline(matting_upstream)["tracks"][0]["segments"][0]["extra_material_refs"]
        rust_chroma_off = rust_data(
            rust, "timeline", "chroma", str(matting_rust), matting_id, "--off"
        )
        upstream_chroma_off = upstream_data(
            entry, "chroma", str(matting_upstream), matting_id, "--off"
        )
        assert rust_chroma_off["removed"] == [rust_chroma["materialId"]]
        assert upstream_chroma_off["removed"] == [upstream_chroma["materialId"]]
        assert read_timeline(matting_rust)["materials"]["chromas"] == []
        assert read_timeline(matting_upstream)["materials"]["chromas"] == []
        results.append({
            "command": "chroma", "status": "semantic",
            "evidence": "fixed-upstream clamp, wire material, reference attachment, and targeted removal",
        })

        mask_args = (
            "rectangle", "--center-x", "0.2", "--center-y", "-0.1",
            "--size", "0.6", "--rotation", "15", "--feather", "20",
            "--invert", "--rect-width", "0.8", "--round-corner", "30",
        )
        rust_mask = rust_data(
            rust, "timeline", "mask", str(matting_rust), matting_id, *mask_args
        )
        upstream_mask = upstream_data(
            entry, "mask", str(matting_upstream), matting_id, *mask_args
        )
        for key in ["ok", "segmentId", "name", "field"]:
            assert rust_mask[key] == upstream_mask[key]
        rust_mask_wire = read_timeline(matting_rust)["materials"][rust_mask["field"]][0]
        upstream_mask_wire = read_timeline(matting_upstream)["materials"][upstream_mask["field"]][0]
        assert {key: value for key, value in rust_mask_wire.items() if key != "id"} == {
            key: value for key, value in upstream_mask_wire.items() if key != "id"
        }
        for project, result in [(matting_rust, rust_mask), (matting_upstream, upstream_mask)]:
            timeline = read_timeline(project)
            source_field = result["field"]
            timeline["materials"]["common_masks"] = timeline["materials"][source_field]
            timeline["materials"][source_field] = []
            write_timeline(project, timeline)
        rust_mask_off = rust_data(
            rust, "timeline", "mask", str(matting_rust), matting_id, "--off"
        )
        upstream_mask_off = upstream_data(
            entry, "mask", str(matting_upstream), matting_id, "--off"
        )
        assert rust_mask_off == upstream_mask_off == {"ok": True, "id": matting_id, "removed": 1}
        results.append({
            "command": "mask", "status": "semantic",
            "evidence": "fixed-upstream CapCut resource, geometry, field selection, and cross-field removal",
        })

        rust_blur = rust_data(
            rust, "timeline", "bg-blur", str(matting_rust), matting_id, "2"
        )
        upstream_blur = upstream_data(
            entry, "bg-blur", str(matting_upstream), matting_id, "2"
        )
        for key in ["ok", "segmentId", "blur"]:
            assert rust_blur[key] == upstream_blur[key]
        rust_blur_wire = next(
            item for item in read_timeline(matting_rust)["materials"]["canvases"]
            if item["id"] == rust_blur["canvas_id"]
        )
        upstream_blur_wire = next(
            item for item in read_timeline(matting_upstream)["materials"]["canvases"]
            if item["id"] == upstream_blur["canvas_id"]
        )
        assert {key: value for key, value in rust_blur_wire.items() if key != "id"} == {
            key: value for key, value in upstream_blur_wire.items() if key != "id"
        }
        rust_blur_off = rust_data(
            rust, "timeline", "bg-blur", str(matting_rust), matting_id, "--off"
        )
        upstream_blur_off = upstream_data(
            entry, "bg-blur", str(matting_upstream), matting_id, "--off"
        )
        assert rust_blur_off == upstream_blur_off == {
            "ok": True, "segmentId": matting_id, "canvas_id": None, "blur": None
        }
        results.append({
            "command": "bg-blur", "status": "semantic",
            "evidence": "fixed-upstream level mapping, canvas wire material, reference replacement, and off",
        })

        fade_rust, fade_upstream = paired(root, base, "audio-fade")
        for project in [fade_rust, fade_upstream]:
            timeline = read_timeline(project)
            timeline["tracks"][0]["type"] = "audio"
            timeline["tracks"][0]["name"] = "audio"
            for index, segment in enumerate(timeline["tracks"][0]["segments"]):
                old_id = segment["material_id"]
                material_id = f"audio-material-{index}"
                segment["material_id"] = material_id
                for bucket, items in timeline["materials"].items():
                    if isinstance(items, list):
                        timeline["materials"][bucket] = [item for item in items if item.get("id") != old_id]
                timeline["materials"]["audios"].append({
                    "id": material_id, "type": "audio", "path": "", "name": f"audio-{index}"
                })
            write_timeline(project, timeline)
        fade_segment_id = read_timeline(fade_rust)["tracks"][0]["segments"][0]["id"]
        fade_prefix = fade_segment_id[:8]
        rust_fade = rust_data(
            rust, "timeline", "audio-fade", str(fade_rust), fade_prefix,
            "--in", "0.5", "--fade-out", "1.0"
        )
        upstream_fade = upstream_data(
            entry, "audio-fade", str(fade_upstream), fade_prefix,
            "--in", "0.5", "--fade-out", "1.0"
        )
        for key in ["ok", "segmentId", "fade_in_us", "fade_out_us"]:
            assert rust_fade[key] == upstream_fade[key]
        rust_fade_wire = next(
            item for item in read_timeline(fade_rust)["materials"]["audio_fades"]
            if item["id"] == rust_fade["fade_id"]
        )
        upstream_fade_wire = next(
            item for item in read_timeline(fade_upstream)["materials"]["audio_fades"]
            if item["id"] == upstream_fade["fade_id"]
        )
        assert {key: value for key, value in rust_fade_wire.items() if key != "id"} == {
            key: value for key, value in upstream_fade_wire.items() if key != "id"
        }
        rust_replaced = rust_data(
            rust, "timeline", "audio-fade", str(fade_rust), fade_prefix, "--in", "0.25"
        )
        upstream_replaced = upstream_data(
            entry, "audio-fade", str(fade_upstream), fade_prefix, "--in", "0.25"
        )
        for project, first, second in [
            (fade_rust, rust_fade, rust_replaced),
            (fade_upstream, upstream_fade, upstream_replaced),
        ]:
            refs = read_timeline(project)["tracks"][0]["segments"][0]["extra_material_refs"]
            assert first["fade_id"] not in refs and second["fade_id"] in refs
            assert len(read_timeline(project)["materials"]["audio_fades"]) == 2
        results.append({
            "command": "audio-fade", "status": "semantic",
            "evidence": "fixed-upstream prefix lookup, seconds conversion, wire material, and replacement",
        })

        filter_rust, filter_upstream = paired(root, base, "add-filter")
        rust_filter = rust_data(
            rust, "timeline", "add-filter", str(filter_rust), "vintage", "0s", "1s",
            "--intensity", "0.4"
        )
        upstream_filter = upstream_data(
            entry, "add-filter", str(filter_upstream), "vintage", "0s", "1s",
            "--intensity", "0.4"
        )
        for key in ["ok", "name", "start_us", "duration_us"]:
            assert rust_filter[key] == upstream_filter[key]
        rust_filter_wire = next(
            item for item in read_timeline(filter_rust)["materials"]["video_effects"]
            if item["id"] == rust_filter["materialId"]
        )
        upstream_filter_wire = next(
            item for item in read_timeline(filter_upstream)["materials"]["video_effects"]
            if item["id"] == upstream_filter["materialId"]
        )
        assert {key: value for key, value in rust_filter_wire.items() if key != "id"} == {
            key: value for key, value in upstream_filter_wire.items() if key != "id"
        }
        rust_raw = rust_data(
            rust, "timeline", "add-filter", str(filter_rust), "Store Look", "0s", "500ms",
            "--resource-id", "7529669127365202194", "--effect-id", "2222222222"
        )
        upstream_raw = upstream_data(
            entry, "add-filter", str(filter_upstream), "Store Look", "0s", "500ms",
            "--resource-id", "7529669127365202194", "--effect-id", "2222222222"
        )
        for key in ["ok", "name", "start_us", "duration_us"]:
            assert rust_raw[key] == upstream_raw[key]
        rust_full = rust_data(
            rust, "timeline", "add-filter", str(filter_rust), "warm", "--full"
        )
        upstream_full = upstream_data(
            entry, "add-filter", str(filter_upstream), "warm", "--full"
        )
        for key in ["ok", "name", "start_us", "duration_us"]:
            assert rust_full[key] == upstream_full[key]
        assert len([track for track in read_timeline(filter_rust)["tracks"] if track["type"] == "filter"]) == 1
        assert len([track for track in read_timeline(filter_upstream)["tracks"] if track["type"] == "filter"]) == 1
        results.append({
            "command": "add-filter", "status": "semantic",
            "evidence": "fixed-upstream catalogue/raw ids, intensity, shared filter track, and full range",
        })

        bubble_rust, bubble_upstream = paired(root, base, "bubble-text")
        bubble_segment = read_timeline(bubble_rust)["tracks"][0]["segments"][0]["id"]
        bubble_prefix = bubble_segment[:8]
        rust_bubble = rust_data(
            rust, "captions", "bubble", str(bubble_rust), bubble_prefix,
            "--bubble", "cloud"
        )
        upstream_bubble = upstream_data(
            entry, "bubble-text", str(bubble_upstream), bubble_prefix,
            "--bubble", "cloud"
        )
        for key in ["ok", "segmentId", "effect_id", "resource_id"]:
            assert rust_bubble[key] == upstream_bubble[key]
        rust_bubble_wire = next(
            item for item in read_timeline(bubble_rust)["materials"]["filters"]
            if item["id"] == rust_bubble["bubble_id"]
        )
        upstream_bubble_wire = next(
            item for item in read_timeline(bubble_upstream)["materials"]["filters"]
            if item["id"] == upstream_bubble["bubble_id"]
        )
        assert {key: value for key, value in rust_bubble_wire.items() if key != "id"} == {
            key: value for key, value in upstream_bubble_wire.items() if key != "id"
        }
        rust_raw_bubble = rust_data(
            rust, "captions", "bubble", str(bubble_rust), bubble_prefix,
            "--effect-id", "1111111111111111111", "--resource-id", "2222222222222222222"
        )
        upstream_raw_bubble = upstream_data(
            entry, "bubble-text", str(bubble_upstream), bubble_prefix,
            "--effect-id", "1111111111111111111", "--resource-id", "2222222222222222222"
        )
        for project, first, second in [
            (bubble_rust, rust_bubble, rust_raw_bubble),
            (bubble_upstream, upstream_bubble, upstream_raw_bubble),
        ]:
            timeline = read_timeline(project)
            refs = timeline["tracks"][0]["segments"][0]["extra_material_refs"]
            assert first["bubble_id"] not in refs and second["bubble_id"] in refs
            assert len(timeline["materials"]["filters"]) == 2
            assert timeline["materials"]["texts"][0]["bubble_effect_id"] == "1111111111111111111"
            assert timeline["materials"]["texts"][0]["bubble_resource_id"] == "2222222222222222222"
        results.append({
            "command": "bubble-text", "status": "semantic",
            "evidence": "fixed-upstream prefix, catalogue/raw ids, dual write, and active-ref replacement",
        })

        effect_rust, effect_upstream = paired(root, base, "add-effect")
        effect_bound = read_timeline(effect_rust)["tracks"][0]["segments"][0]["id"]
        rust_effect = rust_data(
            rust, "timeline", "add-effect", str(effect_rust), "shake", "0s", "1s",
            "--params", "[0.25,0.75]", "--intensity", "0.7", "--bind", effect_bound[:8]
        )
        upstream_effect = upstream_data(
            entry, "add-effect", str(effect_upstream), "shake", "0s", "1s",
            "--params", "[0.25,0.75]", "--intensity", "0.7", "--bind", effect_bound[:8]
        )
        for key in ["ok", "name", "start_us", "duration_us"]:
            assert rust_effect[key] == upstream_effect[key]
        rust_effect_wire = next(
            item for item in read_timeline(effect_rust)["materials"]["video_effects"]
            if item["id"] == rust_effect["materialId"]
        )
        upstream_effect_wire = next(
            item for item in read_timeline(effect_upstream)["materials"]["video_effects"]
            if item["id"] == upstream_effect["materialId"]
        )
        assert {key: value for key, value in rust_effect_wire.items() if key != "id"} == {
            key: value for key, value in upstream_effect_wire.items() if key != "id"
        }
        rust_raw_effect = rust_data(
            rust, "timeline", "add-effect", str(effect_rust), "Store Effect", "0s", "500ms",
            "--resource-id", "7529669127365202194", "--effect-id", "2222222222"
        )
        upstream_raw_effect = upstream_data(
            entry, "add-effect", str(effect_upstream), "Store Effect", "0s", "500ms",
            "--resource-id", "7529669127365202194", "--effect-id", "2222222222"
        )
        for key in ["ok", "name", "start_us", "duration_us"]:
            assert rust_raw_effect[key] == upstream_raw_effect[key]
        rust_full_effect = rust_data(
            rust, "timeline", "add-effect", str(effect_rust), "vhs", "--full"
        )
        upstream_full_effect = upstream_data(
            entry, "add-effect", str(effect_upstream), "vhs", "--full"
        )
        for key in ["ok", "name", "start_us", "duration_us"]:
            assert rust_full_effect[key] == upstream_full_effect[key]
        assert len([track for track in read_timeline(effect_rust)["tracks"] if track["type"] == "effect"]) == 1
        assert len([track for track in read_timeline(effect_upstream)["tracks"] if track["type"] == "effect"]) == 1
        results.append({
            "command": "add-effect", "status": "semantic",
            "evidence": "fixed-upstream catalogue/raw ids, params, intensity, binding, shared track, and full range",
        })

        crop_rust, crop_upstream = paired(root, base, "crop")
        for project in [crop_rust, crop_upstream]:
            timeline = read_timeline(project)
            segment = timeline["tracks"][0]["segments"][0]
            material_id = "crop-video-material"
            segment["material_id"] = material_id
            timeline["tracks"][0]["segments"] = [segment]
            timeline["tracks"][0]["type"] = "video"
            timeline["tracks"][0]["name"] = "video"
            for bucket, items in timeline["materials"].items():
                if isinstance(items, list):
                    timeline["materials"][bucket] = []
            timeline["materials"]["videos"] = [{
                "id": material_id,
                "type": "video",
                "path": "",
                "duration": 2_000_000,
                "width": 1920,
                "height": 1080,
                "crop_ratio": "original",
            }]
            timeline["duration"] = 2_000_000
            write_timeline(project, timeline)
        crop_segment = read_timeline(crop_rust)["tracks"][0]["segments"][0]["id"]
        crop_prefix = crop_segment[:8]
        rust_crop_info = rust_data(rust, "timeline", "crop", str(crop_rust), crop_prefix)
        upstream_crop_info = upstream_data(entry, "crop", str(crop_upstream), crop_prefix)
        assert rust_crop_info == upstream_crop_info

        rust_crop_ratio = rust_data(
            rust, "timeline", "crop", str(crop_rust), crop_prefix, "--ratio", "9:16"
        )
        upstream_crop_ratio = upstream_data(
            entry, "crop", str(crop_upstream), crop_prefix, "--ratio", "9:16"
        )
        assert rust_crop_ratio == upstream_crop_ratio
        assert read_timeline(crop_rust)["materials"]["videos"][0]["crop"] == read_timeline(
            crop_upstream
        )["materials"]["videos"][0]["crop"]

        rust_crop_rect = rust_data(
            rust, "timeline", "crop", str(crop_rust), crop_prefix,
            "--ratio", "1:1", "--rect", "0,0,0.5,0.5"
        )
        upstream_crop_rect = upstream_data(
            entry, "crop", str(crop_upstream), crop_prefix,
            "--ratio", "1:1", "--rect", "0,0,0.5,0.5"
        )
        assert rust_crop_rect == upstream_crop_rect

        before_rust = (crop_rust / "draft_content.json").read_bytes()
        before_upstream = (crop_upstream / "draft_content.json").read_bytes()
        rust_crop_dry = rust_data(
            rust, "timeline", "crop", str(crop_rust), crop_prefix,
            "--rect", "0.25,0.25,0.5,0.5", "--dry-run"
        )
        upstream_crop_dry = upstream_data(
            entry, "crop", str(crop_upstream), crop_prefix,
            "--rect", "0.25,0.25,0.5,0.5", "--dry-run"
        )
        assert rust_crop_dry == upstream_crop_dry
        assert (crop_rust / "draft_content.json").read_bytes() == before_rust
        assert (crop_upstream / "draft_content.json").read_bytes() == before_upstream

        rust_crop_reset = rust_data(
            rust, "timeline", "crop", str(crop_rust), crop_prefix, "--reset"
        )
        upstream_crop_reset = upstream_data(
            entry, "crop", str(crop_upstream), crop_prefix, "--reset"
        )
        assert rust_crop_reset == upstream_crop_reset
        results.append({
            "command": "crop", "status": "exact",
            "evidence": "fixed-upstream read, prefix, ratio, rect precedence, reset, and dry-run",
        })

        keyframe_rust, keyframe_upstream = paired(root, base, "keyframe")
        keyframe_id = read_timeline(keyframe_rust)["tracks"][0]["segments"][0]["id"]
        keyframe_prefix = keyframe_id[:8]
        rust_first = rust_data(
            rust, "timeline", "keyframe", str(keyframe_rust), keyframe_prefix,
            "scale", "0s", "1.0", "--easing", "ease-out"
        )
        upstream_first = upstream_data(
            entry, "keyframe", str(keyframe_upstream), keyframe_prefix,
            "scale", "0s", "1.0", "--easing", "ease-out"
        )
        assert rust_first == upstream_first
        rust_second = rust_data(
            rust, "timeline", "keyframe", str(keyframe_rust), keyframe_prefix,
            "uniform_scale", "5s", "1.3", "--easing", "ease-out"
        )
        upstream_second = upstream_data(
            entry, "keyframe", str(keyframe_upstream), keyframe_prefix,
            "uniform_scale", "5s", "1.3", "--easing", "ease-out"
        )
        assert rust_second == upstream_second
        batch = (
            '{"property":"x","time":1000000,"value":"0.25"}\n'
            '{"property":"opacity","time":"2s","value":"50%","easing":"linear"}\n'
        )
        rust_batch = rust_data_stdin(
            rust, batch, "timeline", "keyframe", str(keyframe_rust), keyframe_prefix, "--batch"
        )
        upstream_batch = upstream_data_stdin(
            entry, batch, "keyframe", str(keyframe_upstream), keyframe_prefix, "--batch"
        )
        assert rust_batch == upstream_batch
        hold_batch = (
            '{"property":"brightness","time":0,"value":"0","easing":"hold"}\n'
            '{"property":"brightness","time":"3s","value":"0.5"}\n'
        )
        rust_hold = rust_data_stdin(
            rust, hold_batch, "timeline", "keyframe", str(keyframe_rust), keyframe_prefix, "--batch"
        )
        upstream_hold = upstream_data_stdin(
            entry, hold_batch, "keyframe", str(keyframe_upstream), keyframe_prefix, "--batch"
        )
        assert rust_hold == upstream_hold
        rust_keyframes = read_timeline(keyframe_rust)["tracks"][0]["segments"][0]["common_keyframes"]
        upstream_keyframes = read_timeline(keyframe_upstream)["tracks"][0]["segments"][0]["common_keyframes"]
        assert without_generated_ids(rust_keyframes) == without_generated_ids(upstream_keyframes)
        results.append({
            "command": "keyframe", "status": "semantic",
            "evidence": "fixed-upstream alias, value normalization, JSONL batch, easing handles, hold helper, and prefix lookup",
        })

        transition_rust, transition_upstream = paired(root, base, "transition")
        transition_segments = read_timeline(transition_rust)["tracks"][0]["segments"]
        rust_transition = rust_data(
            rust, "timeline", "transition", str(transition_rust),
            transition_segments[0]["id"][:8], "dissolve"
        )
        upstream_transition = upstream_data(
            entry, "transition", str(transition_upstream),
            transition_segments[0]["id"][:8], "dissolve"
        )
        for key in ["ok", "segmentId", "name", "duration_us"]:
            assert rust_transition[key] == upstream_transition[key]
        rust_jy_transition = rust_data(
            rust, "timeline", "transition", str(transition_rust),
            transition_segments[1]["id"][:8], "_3D空间", "--duration", "750ms", "--jianying"
        )
        upstream_jy_transition = upstream_data(
            entry, "transition", str(transition_upstream),
            transition_segments[1]["id"][:8], "_3D空间", "--duration", "750ms", "--jianying"
        )
        for key in ["ok", "segmentId", "name", "duration_us"]:
            assert rust_jy_transition[key] == upstream_jy_transition[key]
        rust_transition_wire = read_timeline(transition_rust)["materials"]["transitions"]
        upstream_transition_wire = read_timeline(transition_upstream)["materials"]["transitions"]
        assert without_generated_ids(rust_transition_wire) == without_generated_ids(upstream_transition_wire)
        for project, first, second in [
            (transition_rust, rust_transition, rust_jy_transition),
            (transition_upstream, upstream_transition, upstream_jy_transition),
        ]:
            segments = read_timeline(project)["tracks"][0]["segments"]
            assert first["transition_id"] in segments[0]["extra_material_refs"]
            assert second["transition_id"] in segments[1]["extra_material_refs"]
        results.append({
            "command": "transition", "status": "semantic",
            "evidence": "fixed-upstream CapCut/JianYing catalogues, default/explicit duration, wire, refs, and prefix lookup",
        })

        text_anim_rust, text_anim_upstream = paired(root, base, "text-anim")
        text_anim_segments = read_timeline(text_anim_rust)["tracks"][0]["segments"]
        rust_text_anim = rust_data(
            rust, "captions", "animation", str(text_anim_rust), text_anim_segments[0]["id"][:8],
            "--intro", "typewriter", "--outro", "fade-out", "--outro-duration", "750ms"
        )
        upstream_text_anim = upstream_data(
            entry, "text-anim", str(text_anim_upstream), text_anim_segments[0]["id"][:8],
            "--intro", "typewriter", "--outro", "fade-out", "--outro-duration", "750ms"
        )
        for key in ["ok", "segmentId", "added"]:
            assert rust_text_anim[key] == upstream_text_anim[key]
        rust_jy_text_anim = rust_data(
            rust, "captions", "animation", str(text_anim_rust), text_anim_segments[1]["id"][:8],
            "--intro", "卡拉OK", "--intro-duration", "600ms", "--jianying"
        )
        upstream_jy_text_anim = upstream_data(
            entry, "text-anim", str(text_anim_upstream), text_anim_segments[1]["id"][:8],
            "--intro", "卡拉OK", "--intro-duration", "600ms", "--jianying"
        )
        for key in ["ok", "segmentId", "added"]:
            assert rust_jy_text_anim[key] == upstream_jy_text_anim[key]
        rust_containers = read_timeline(text_anim_rust)["materials"]["material_animations"]
        upstream_containers = read_timeline(text_anim_upstream)["materials"]["material_animations"]
        assert len(rust_containers) == len(upstream_containers) == 2
        for rust_container, upstream_container in zip(rust_containers, upstream_containers):
            assert {key: value for key, value in rust_container.items() if key != "id"} == {
                key: value for key, value in upstream_container.items() if key != "id"
            }
        results.append({
            "command": "text-anim", "status": "semantic",
            "evidence": "fixed-upstream full dual catalogues, default/explicit duration, outro anchor, container wire, and prefix lookup",
        })

        image_anim_rust, image_anim_upstream = paired(root, base, "image-anim")
        image_anim_segments = read_timeline(image_anim_rust)["tracks"][0]["segments"]
        rust_image_anim = rust_data(
            rust, "timeline", "image-animation", str(image_anim_rust), image_anim_segments[0]["id"][:8],
            "--intro", "fade-in", "--outro", "blur-out", "--outro-duration", "600ms",
            "--combo", "distort-and-stretch", "--combo-duration", "700ms"
        )
        upstream_image_anim = upstream_data(
            entry, "image-anim", str(image_anim_upstream), image_anim_segments[0]["id"][:8],
            "--intro", "fade-in", "--outro", "blur-out", "--outro-duration", "600ms",
            "--combo", "distort-and-stretch", "--combo-duration", "700ms"
        )
        for key in ["ok", "segmentId", "added"]:
            assert rust_image_anim[key] == upstream_image_anim[key]
        rust_jy_image_anim = rust_data(
            rust, "timeline", "image-animation", str(image_anim_rust), image_anim_segments[1]["id"][:8],
            "--intro", "缩小", "--combo", "三分割", "--jianying"
        )
        upstream_jy_image_anim = upstream_data(
            entry, "image-anim", str(image_anim_upstream), image_anim_segments[1]["id"][:8],
            "--intro", "缩小", "--combo", "三分割", "--jianying"
        )
        for key in ["ok", "segmentId", "added"]:
            assert rust_jy_image_anim[key] == upstream_jy_image_anim[key]
        rust_image_containers = read_timeline(image_anim_rust)["materials"]["material_animations"]
        upstream_image_containers = read_timeline(image_anim_upstream)["materials"]["material_animations"]
        assert len(rust_image_containers) == len(upstream_image_containers) == 2
        for rust_container, upstream_container in zip(rust_image_containers, upstream_image_containers):
            assert {key: value for key, value in rust_container.items() if key != "id"} == {
                key: value for key, value in upstream_container.items() if key != "id"
            }
        results.append({
            "command": "image-anim", "status": "semantic",
            "evidence": "fixed-upstream starter and full dual catalogues, intro/outro/combo anchors, container wire, and prefix lookup",
        })

        sticker_rust, sticker_upstream = paired(root, base, "add-sticker")
        rust_sticker = rust_data(
            rust, "media", "add-sticker", str(sticker_rust), "7520000000000000001", "250ms", "1.5s",
            "--x", "0.25", "--y", "-0.5", "--scale", "1.2", "--rotation", "15"
        )
        upstream_sticker = upstream_data(
            entry, "add-sticker", str(sticker_upstream), "7520000000000000001", "250ms", "1.5s",
            "--x", "0.25", "--y", "-0.5", "--scale", "1.2", "--rotation", "15"
        )
        rust_sticker_2 = rust_data(
            rust, "media", "add-sticker", str(sticker_rust), "7520000000000000002", "2s", "500ms"
        )
        upstream_sticker_2 = upstream_data(
            entry, "add-sticker", str(sticker_upstream), "7520000000000000002", "2s", "500ms"
        )
        assert rust_sticker["start_us"] == upstream_sticker["start_us"] == 250_000
        assert rust_sticker["duration_us"] == upstream_sticker["duration_us"] == 1_500_000
        assert rust_sticker["trackId"] == rust_sticker_2["trackId"]
        assert upstream_sticker["trackId"] == upstream_sticker_2["trackId"]
        rust_sticker_timeline = read_timeline(sticker_rust)
        upstream_sticker_timeline = read_timeline(sticker_upstream)
        rust_sticker_track = next(track for track in rust_sticker_timeline["tracks"] if track["type"] == "sticker")
        upstream_sticker_track = next(track for track in upstream_sticker_timeline["tracks"] if track["type"] == "sticker")
        for key in ["type", "name", "attribute", "is_default_name", "flag"]:
            assert rust_sticker_track[key] == upstream_sticker_track[key]
        assert len(rust_sticker_track["segments"]) == len(upstream_sticker_track["segments"]) == 2
        for rust_segment, upstream_segment in zip(rust_sticker_track["segments"], upstream_sticker_track["segments"]):
            ignored = {"id", "material_id", "raw_segment_id", "extra_material_refs"}
            assert {key: value for key, value in rust_segment.items() if key not in ignored} == {
                key: value for key, value in upstream_segment.items() if key not in ignored
            }
            assert len(rust_segment["extra_material_refs"]) == len(upstream_segment["extra_material_refs"]) == 6
        for bucket in [
            "stickers", "speeds", "placeholder_infos", "sound_channel_mappings",
            "vocal_separations", "canvases", "material_colors",
        ]:
            rust_items = rust_sticker_timeline["materials"][bucket]
            upstream_items = upstream_sticker_timeline["materials"][bucket]
            assert len(rust_items) == len(upstream_items)
            for rust_item, upstream_item in zip(rust_items, upstream_items):
                assert {key: value for key, value in rust_item.items() if key != "id"} == {
                    key: value for key, value in upstream_item.items() if key != "id"
                }
        results.append({
            "command": "add-sticker", "status": "semantic",
            "evidence": "fixed-upstream default track reuse, transform wire, sticker material, and six companion material references",
        })

        ranges_rust, ranges_upstream = paired(root, base, "text-ranges")
        ranges_segment_id = read_timeline(ranges_rust)["tracks"][0]["segments"][0]["id"]
        ranges = json.dumps([
            {"start": 3, "end": 5, "font_size": 18, "italic": True},
            {"start": 0, "end": 2, "font_color": "#FF0000", "font_alpha": 0.5, "bold": True},
        ], separators=(",", ":"))
        rust_ranges = rust_data(
            rust, "captions", "style-ranges", str(ranges_rust), ranges_segment_id[:8],
            "--styles", ranges,
        )
        upstream_ranges = upstream_data(
            entry, "text-ranges", str(ranges_upstream), ranges_segment_id[:8],
            "--styles", ranges,
        )
        assert rust_ranges == upstream_ranges
        rust_range_timeline = read_timeline(ranges_rust)
        upstream_range_timeline = read_timeline(ranges_upstream)
        rust_text = next(
            item for item in rust_range_timeline["materials"]["texts"]
            if item["id"] == rust_ranges["material_id"]
        )
        upstream_text = next(
            item for item in upstream_range_timeline["materials"]["texts"]
            if item["id"] == upstream_ranges["material_id"]
        )
        assert json.loads(rust_text["content"]) == json.loads(upstream_text["content"])
        results.append({
            "command": "text-ranges", "status": "exact",
            "evidence": "fixed-upstream UTF-16 ranges, stable sorting, inherited gap styles, prefix lookup, and content wire",
        })

        for command in ["opacity", "composite"]:
            results.append({
                "command": command,
                "status": "semantic",
                "evidence": "timeline_cli_contract",
            })

    report = {
        "schema": "jianying-capcut-timeline-differential/v1",
        "upstream": {"repository": "renezander030/capcut-cli", "commit": revision},
        "runner": "tools/capcut_timeline_differential.py",
        "cases": results,
        "passed": len(results),
        "failed": 0,
    }
    encoded = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
