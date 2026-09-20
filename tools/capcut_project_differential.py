#!/usr/bin/env python3
"""Black-box project-command differential against pinned capcut-cli.

The upstream checkout is supplied explicitly and must resolve to commit
49f70e3b. The script never imports upstream source; it invokes both CLIs as
processes and compares observable JSON and generated draft semantics.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import shutil
import subprocess
import tempfile
from typing import Any

PINNED = "49f70e3b07f1a236d45acb9b49a70c141dd1ee98"


def invoke(argv: list[str]) -> Any:
    process = subprocess.run(argv, text=True, capture_output=True, check=False)
    if process.returncode != 0:
        raise RuntimeError(
            f"command failed ({process.returncode}): {argv!r}\n"
            f"stdout={process.stdout}\nstderr={process.stderr}"
        )
    # Pinned upstream 49f70e3b contains one literal NUL in a generated help
    # example; Node emits it inside describe JSON. Lenient decoding preserves
    # the black-box payload while still validating its structure.
    return json.loads(process.stdout, strict=False)


def rust_data(binary: pathlib.Path, *args: str) -> Any:
    return invoke([str(binary), *args, "--json"])["data"]


def upstream_data(entry: pathlib.Path, *args: str) -> Any:
    return invoke(["node", str(entry), *args])


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
    if not entry.is_file():
        raise RuntimeError("upstream dist/index.js is missing; run npm ci && npm run build")
    rust = args.rust_bin.resolve()
    if not rust.is_file():
        raise RuntimeError(f"Rust binary is missing: {rust}")

    results: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory(prefix="jianying-capcut-project-diff-") as temporary:
        root = pathlib.Path(temporary)
        base = root / "base"
        rust_data(rust, "project", "init", "sample", "--out", str(base))

        rust_info = rust_data(rust, "project", "info", str(base))
        upstream_info = upstream_data(entry, "info", str(base))
        assert rust_info == upstream_info
        results.append({"command": "info", "status": "exact"})

        changed = root / "changed"
        shutil.copytree(base, changed)
        changed_timeline = read_timeline(changed)
        changed_timeline["tracks"][0]["segments"] = [
            {
                "id": "added-segment",
                "material_id": "added-material",
                "target_timerange": {"start": 10, "duration": 20},
                "speed": 1.0,
                "volume": 1.0,
            }
        ]
        changed_timeline["materials"]["texts"] = [
            {"id": "added-material", "content": "added"}
        ]
        write_timeline(changed, changed_timeline)
        rust_diff = rust_data(rust, "project", "diff", str(base), str(changed))
        upstream_diff = upstream_data(entry, "diff", str(base), str(changed))
        assert rust_diff == upstream_diff
        results.append({"command": "diff", "status": "exact"})

        rust_migrate = root / "rust-migrate"
        upstream_migrate = root / "upstream-migrate"
        shutil.copytree(base, rust_migrate)
        shutil.copytree(base, upstream_migrate)
        for project in [rust_migrate, upstream_migrate]:
            timeline = read_timeline(project)
            timeline["materials"]["masks"] = [{"id": "legacy-mask"}]
            write_timeline(project, timeline)
        rust_migration = rust_data(
            rust, "project", "migrate", str(rust_migrate), "--from", "5.9", "--to", "9.6"
        )
        upstream_migration = upstream_data(
            entry, "migrate", str(upstream_migrate), "--from", "5.9", "--to", "9.6"
        )
        assert rust_migration == upstream_migration
        assert read_timeline(rust_migrate)["materials"] == read_timeline(upstream_migrate)["materials"]
        results.append({"command": "migrate", "status": "exact"})

        srt_a = root / "a.srt"
        srt_b = root / "b.srt"
        srt_a.write_text("1\n00:00:00,000 --> 00:00:01,000\nA\n", encoding="utf-8")
        srt_b.write_text("1\n00:00:00,000 --> 00:00:02,000\nB\n", encoding="utf-8")
        draft_a = root / "a"
        draft_b = root / "b"
        rust_data(rust, "project", "quickstart", "a", "--out", str(draft_a), "--srt", str(srt_a))
        rust_data(rust, "project", "quickstart", "b", "--out", str(draft_b), "--srt", str(srt_b))
        rust_combined = root / "rust-combined"
        upstream_combined = root / "upstream-combined.json"
        rust_concat = rust_data(
            rust, "project", "concat", str(draft_a), str(draft_b), "--out", str(rust_combined)
        )
        upstream_concat = upstream_data(
            entry, "concat", str(draft_a), str(draft_b), "--out", str(upstream_combined)
        )
        assert rust_concat["duration_us"] == upstream_concat["duration_us"]
        assert rust_concat["remapped_ids"] == upstream_concat["remapped_ids"]
        assert read_timeline(rust_combined) == json.loads(upstream_combined.read_text(encoding="utf-8"))
        results.append({"command": "concat", "status": "exact-timeline"})

        cut_srt = root / "cut.srt"
        cut_srt.write_text(
            "1\n00:00:00,000 --> 00:00:01,000\none\n\n"
            "2\n00:00:01,000 --> 00:00:02,000\ntwo\n\n"
            "3\n00:00:02,000 --> 00:00:03,000\nthree\n",
            encoding="utf-8",
        )
        cut_source = root / "cut-source"
        rust_data(
            rust, "project", "quickstart", "cut-source", "--out", str(cut_source),
            "--srt", str(cut_srt)
        )
        cut_source_before = (cut_source / "draft_content.json").read_bytes()
        rust_cut_out = root / "rust-cut.json"
        upstream_cut_out = root / "upstream-cut.json"
        rust_cut = rust_data(
            rust, "project", "cut", str(cut_source), "500ms", "1500ms",
            "--out", str(rust_cut_out)
        )
        upstream_cut = upstream_data(
            entry, "cut", str(cut_source), "500ms", "1500ms",
            "--out", str(upstream_cut_out)
        )
        for key in ["ok", "kept", "removed", "duration_us"]:
            assert rust_cut[key] == upstream_cut[key]
        assert json.loads(rust_cut_out.read_text(encoding="utf-8")) == json.loads(
            upstream_cut_out.read_text(encoding="utf-8")
        )
        assert (cut_source / "draft_content.json").read_bytes() == cut_source_before
        results.append({
            "command": "cut", "status": "exact-timeline",
            "evidence": "fixed-upstream clipping, rebasing, source range, orphan cleanup, and source isolation",
        })

        rust_prune = root / "rust-prune"
        upstream_prune = root / "upstream-prune"
        shutil.copytree(draft_a, rust_prune)
        shutil.copytree(draft_a, upstream_prune)
        for project in [rust_prune, upstream_prune]:
            timeline = read_timeline(project)
            timeline["materials"]["effects"] = [
                {"id": "kept-companion", "name": "referenced"},
                {"id": "orphan-effect", "name": "orphan"},
                {"name": "anonymous entry"},
            ]
            timeline["tracks"][0]["segments"][0].setdefault(
                "extra_material_refs", []
            ).append("kept-companion")
            write_timeline(project, timeline)
        rust_pruned = rust_data(rust, "project", "prune", str(rust_prune))
        upstream_pruned = upstream_data(entry, "prune", str(upstream_prune))
        assert rust_pruned == upstream_pruned
        assert read_timeline(rust_prune)["materials"] == read_timeline(upstream_prune)["materials"]
        assert [item.get("id") for item in read_timeline(rust_prune)["materials"]["effects"]] == [
            "kept-companion",
            None,
        ]
        results.append({"command": "prune", "status": "exact"})

        cover_image = root / "cover.png"
        cover_image.write_bytes(b"fixed-cover-fixture")
        rust_cover = root / "rust-cover"
        upstream_cover = root / "upstream-cover"
        shutil.copytree(base, rust_cover)
        shutil.copytree(base, upstream_cover)
        rust_cover_output = rust_data(
            rust, "project", "add-cover", str(rust_cover), str(cover_image), "--time", "1500"
        )
        upstream_cover_output = upstream_data(
            entry, "add-cover", str(upstream_cover), str(cover_image), "--time", "1500"
        )
        assert rust_cover_output == upstream_cover_output
        rust_cover_wire = read_timeline(rust_cover)["cover"]
        upstream_cover_wire = read_timeline(upstream_cover)["cover"]
        assert isinstance(rust_cover_wire["custom_cover_id"], str)
        assert isinstance(upstream_cover_wire["custom_cover_id"], str)
        assert rust_cover_wire["custom_cover_id"]
        assert upstream_cover_wire["custom_cover_id"]
        assert {
            key: value for key, value in rust_cover_wire.items() if key != "custom_cover_id"
        } == {
            key: value for key, value in upstream_cover_wire.items() if key != "custom_cover_id"
        }
        rust_cover_default = rust_data(
            rust, "project", "add-cover", str(rust_cover), str(cover_image)
        )
        upstream_cover_default = upstream_data(
            entry, "add-cover", str(upstream_cover), str(cover_image)
        )
        assert rust_cover_default == upstream_cover_default
        assert read_timeline(rust_cover)["cover"]["time_ms"] == 0
        assert read_timeline(upstream_cover)["cover"]["time_ms"] == 0
        results.append({"command": "add-cover", "status": "semantic", "approved": ["generated-id"]})

        upstream_store = root / "upstream-store"
        upstream_store.mkdir()
        upstream_init = upstream_data(
            entry, "init", "upstream-init", "--drafts", str(upstream_store), "--template", "bundled"
        )
        rust_init = rust_data(rust, "project", "init", "rust-init", "--out", str(root / "rust-init"))
        upstream_canvas = upstream_init["canvas"] or read_timeline(
            pathlib.Path(upstream_init["draft_path"])
        )["canvas_config"]
        assert upstream_canvas["width"] == rust_init["canvas"]["width"]
        assert upstream_canvas["height"] == rust_init["canvas"]["height"]
        results.append({"command": "init", "status": "semantic", "approved": ["template", "registration"]})

        upstream_quick = upstream_data(
            entry,
            "quickstart",
            "upstream-quick",
            "--srt",
            str(srt_a),
            "--drafts",
            str(upstream_store),
            "--template",
            "bundled",
        )
        rust_quick = rust_data(
            rust,
            "project",
            "quickstart",
            "rust-quick",
            "--out",
            str(root / "rust-quick"),
            "--srt",
            str(srt_a),
        )
        assert upstream_quick["added"] == rust_quick["added"]
        assert upstream_quick["ok"] == rust_quick["ok"]
        results.append({"command": "quickstart", "status": "semantic", "approved": ["template", "registration", "diagnostics"]})

        upstream_version = upstream_data(entry, "version", str(base))
        rust_version = rust_data(rust, "project", "version", str(base))
        assert upstream_version["app"] == rust_version["app"]
        assert upstream_version["app_version"] == rust_version["app_version"]
        assert upstream_version["schema"] == rust_version["schema"]
        assert rust_version["support"]["write_guard"] == "block"
        results.append({"command": "version", "status": "approved-difference", "reason": "Runtime Profile gate remains fail-closed"})

        describe_output = root / "upstream-describe.json"
        with describe_output.open("wb") as stream:
            describe_process = subprocess.run(
                ["node", str(entry), "describe"], stdout=stream, stderr=subprocess.PIPE, check=False
            )
        if describe_process.returncode != 0:
            raise RuntimeError(describe_process.stderr.decode("utf-8", errors="replace"))
        describe_text = describe_output.read_text(encoding="utf-8", errors="replace")
        rust_describe = rust_data(rust, "project", "describe")
        rust_paths = {item["path"] for item in rust_describe["commands"]}
        for required in ["info", "version", "diff", "concat", "cut", "migrate", "init", "quickstart", "prune", "add-cover"]:
            assert f'"name":"{required}"' in describe_text
            assert f"project {required}" in rust_paths
        results.append({"command": "describe", "status": "semantic-command-map"})

    report = {
        "schema": "jianying-capcut-project-differential/v1",
        "upstream": {"repository": "renezander030/capcut-cli", "commit": revision},
        "runner": "tools/capcut_project_differential.py",
        "rust_cli": "provided-by-runner",
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
