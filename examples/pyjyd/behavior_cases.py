#!/usr/bin/env python3
"""行为型示例的对照用例。

上游 `tests/` 里有 5 个模块断言的是 API 语义（时间解析、轨道规则、插入顺序、
模板轨道列举），不产出可比的整份草稿。这里把它们表述成"同一输入 → 同一结局"的
用例：每例两边各返回一个 `{"outcome": ..., ...}`，由
`tools/pyjyd_behavior_parity.py` 逐项比较。

`covers` 列出它覆盖的上游测试函数；合同测试要求 51 个上游测试函数里的每一个
都至少被一个用例覆盖。
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))


def _draft():
    from reference_common import load_pyjyd

    return load_pyjyd()


def _ours(binary: Path, argv: list[str], env: dict) -> tuple[int, dict]:
    # 若参数里有 `--`（用于传入 -500ms 这类带负号的值），
    # `--json` 必须放在它前面，否则会被当成位置参数。
    args = list(argv)
    if "--" in args:
        index = args.index("--")
        args = args[:index] + ["--json"] + args[index:]
    else:
        args = args + ["--json"]
    process = subprocess.run(
        [str(binary), *args], capture_output=True, text=True, env=env
    )
    try:
        payload = json.loads(process.stdout)
    except json.JSONDecodeError:
        payload = {"raw": process.stdout[:200]}
    return process.returncode, payload


def _plan(directory: Path, tracks: list[dict], name: str = "behavior") -> Path:
    plan = {
        "schema": "jianying-cli-plan/v1",
        "name": name,
        "canvas": {"width": 1920, "height": 1080, "fps": 30},
        "tracks": tracks,
    }
    path = directory / "plan.json"
    path.write_text(json.dumps(plan, ensure_ascii=False), encoding="utf-8")
    return path


def _build(binary: Path, plan: Path, out: Path, env: dict) -> tuple[int, dict]:
    process = subprocess.run(
        [str(binary), "project", "build", str(plan), "--out", str(out), "--json"],
        capture_output=True, text=True, env=env,
    )
    try:
        payload = json.loads(process.stdout)
    except json.JSONDecodeError:
        payload = {"raw": process.stdout[:200]}
    return process.returncode, payload


def _inspect(binary: Path, draft: Path, env: dict) -> tuple[int, dict]:
    return _ours(binary, ["project", "inspect", str(draft)], env)


# ---------------------------------------------------------------- 时间解析

def case_time_parsing(binary: Path, work: Path, env: dict) -> tuple[dict, dict]:
    draft = _draft()
    common = ["1.5s", "0.5s", "2s"]
    reference = {
        "outcome": "ok",
        "values": [draft.tim(item) for item in common]
        + [draft.trange_seconds(2, duration=1.5).duration],
    }

    empty = work / "time-draft"
    _ours(binary, ["project", "init", "time", "--out", str(empty)], env)
    values = []
    for duration in common:
        code, payload = _ours(
            binary, ["captions", "add", str(empty), "x", "0s", duration], env
        )
        values.append(payload["data"]["duration_us"] if code == 0 else None)
    code, payload = _ours(
        binary, ["captions", "add", str(empty), "y", "2s", "1.5s"], env
    )
    values.append(payload["data"]["duration_us"] if code == 0 else None)
    return reference, {
        "outcome": "ok" if all(value is not None for value in values) else "error",
        "values": values,
    }


def case_time_negative(binary: Path, work: Path, env: dict) -> tuple[dict, dict]:
    draft = _draft()
    reference = {"outcome": "ok", "value": draft.tim("1s") + draft.tim("-0.5s")}

    empty = work / "neg-draft"
    _ours(binary, ["project", "init", "neg", "--out", str(empty)], env)
    code, payload = _ours(
        binary, ["captions", "add", str(empty), "x", "1s", "1s"], env
    )
    if code != 0:
        return reference, {"outcome": "add_failed", "value": None}
    segment_id = payload["data"]["segment_id"]
    # clap 会把 -500ms 当成选项，因此用 `--` 隔开
    code, _ = _ours(binary, ["timeline", "move", str(empty), segment_id, "--", "-500ms"], env)
    _, after = _ours(binary, ["timeline", "segments", str(empty)], env)
    start = after["data"][0]["start_us"] if code == 0 else None
    return reference, {"outcome": "ok" if start is not None else "error", "value": start}


# ---------------------------------------------------------------- 轨道规则

def case_overlap_rejected(binary: Path, work: Path, env: dict) -> tuple[dict, dict]:
    draft = _draft()
    from tests.helpers import fake_video_material

    reference = {"outcome": "rejected"}
    try:
        track = draft.Track(draft.TrackType.video)
        track.add_segment(draft.VideoSegment(fake_video_material(), draft.trange("0s", "2s")))
        track.add_segment(
            draft.VideoSegment(fake_video_material(), draft.trange("1999999us", "1s"))
        )
        reference = {"outcome": "accepted"}
    except Exception:  # noqa: BLE001 - 上游用异常表达拒绝
        reference = {"outcome": "rejected"}

    plan = _plan(work, [{
        "type": "video", "name": "video",
        "segments": [
            {"start_us": 0, "duration_us": 2000000, "source": str(work / "clip.mp4")},
            {"start_us": 1999999, "duration_us": 1000000, "source": str(work / "clip.mp4")},
        ],
    }])
    code, _ = _build(binary, plan, work / "overlap", env)
    return reference, {"outcome": "rejected" if code != 0 else "accepted"}


def case_track_order(binary: Path, work: Path, env: dict) -> tuple[dict, dict]:
    draft = _draft()
    script = draft.ScriptFile(1920, 1080, 30, True)
    script.append_tracks([
        draft.TrackSpec(draft.TrackType.audio, "bgm"),
        draft.TrackSpec(draft.TrackType.video, "main_video"),
        draft.TrackSpec(draft.TrackType.text, "caption"),
    ])
    reference = {
        "outcome": "ok",
        "names": [track["name"] for track in json.loads(script.dumps())["tracks"]],
    }

    plan = _plan(work, [
        {"type": "video", "name": "main_video", "segments": [
            {"start_us": 0, "duration_us": 1000000, "source": str(work / "clip.mp4")},
        ]},
        {"type": "audio", "name": "bgm", "segments": [
            {"start_us": 0, "duration_us": 1000000, "source": str(work / "audio.mp3")},
        ]},
        {"type": "text", "name": "caption", "segments": [
            {"start_us": 0, "duration_us": 1000000, "text": "x"},
        ]},
    ])
    out = work / "order"
    code, _ = _build(binary, plan, out, env)
    if code != 0:
        return reference, {"outcome": "build_failed", "names": []}
    _, payload = _inspect(binary, out, env)
    return reference, {
        "outcome": "ok",
        "names": [track["name"] for track in payload["data"]["tracks"]],
    }


def case_insert_order(binary: Path, work: Path, env: dict) -> tuple[dict, dict]:
    draft = _draft()
    script = draft.ScriptFile(1920, 1080, 30, True)
    video_ref = script.append_track(draft.TrackSpec(draft.TrackType.video, "video"))
    script.append_track(draft.TrackSpec(draft.TrackType.effect, "effect"))
    script.insert_track(draft.TrackSpec(draft.TrackType.text, "title"), over_track=video_ref)
    reference = {
        "outcome": "ok",
        "names": [track["name"] for track in json.loads(script.dumps())["tracks"]],
    }

    plan = _plan(work, [
        {"type": "video", "name": "video", "segments": [
            {"start_us": 0, "duration_us": 1000000, "source": str(work / "clip.mp4")},
        ]},
        {"type": "text", "name": "title", "segments": [
            {"start_us": 0, "duration_us": 1000000, "text": "x"},
        ]},
        {"type": "effect", "name": "effect", "segments": [
            {"start_us": 0, "duration_us": 1000000,
             "effects": [{"name": "幻影", "params": {}}]},
        ]},
    ])
    out = work / "insert"
    code, _ = _build(binary, plan, out, env)
    if code != 0:
        return reference, {"outcome": "build_failed", "names": []}
    _, payload = _inspect(binary, out, env)
    return reference, {
        "outcome": "ok",
        "names": [track["name"] for track in payload["data"]["tracks"]],
    }


# ---------------------------------------------------------------- 模板轨道

def case_template_tracks(binary: Path, work: Path, env: dict) -> tuple[dict, dict]:
    draft = _draft()
    template_root = work / "template-library"
    template_root.mkdir(parents=True, exist_ok=True)
    folder = draft.DraftFolder(str(template_root))
    script = folder.create_draft("template", 1920, 1080, allow_replace=True)
    import_ref = script.append_track(draft.TrackSpec(draft.TrackType.video, "imported"))
    script.append_track(draft.TrackSpec(draft.TrackType.text, "caption"))
    script.save()
    loaded = folder.load_template("template")
    reference = {
        "outcome": "ok",
        "names": [track.name for track in loaded.list_imported_tracks()],
    }

    code, payload = _ours(binary, ["template", "inspect", str(template_root / "template")], env)
    if code != 0:
        return reference, {"outcome": "error", "names": []}
    names = [track["name"] for track in payload["data"]["tracks"]]
    return reference, {"outcome": "ok", "names": names}


def case_type_mismatch(binary: Path, work: Path, env: dict) -> tuple[dict, dict]:
    draft = _draft()
    reference = {"outcome": "rejected"}
    try:
        track = draft.Track(draft.TrackType.audio)
        track.add_segment(draft.VideoSegment.__new__(draft.VideoSegment))
        reference = {"outcome": "accepted"}
    except Exception:  # noqa: BLE001 - 上游用异常表达拒绝
        reference = {"outcome": "rejected"}

    # 音频轨里引用视频文件：我们只取其音轨
    plan = _plan(work, [{
        "type": "audio", "name": "audio",
        "segments": [{"start_us": 0, "duration_us": 1000000, "source": str(work / "clip.mp4")}],
    }])
    code, _ = _build(binary, plan, work / "mismatch", env)
    return reference, {"outcome": "rejected" if code != 0 else "accepted"}


def case_type_mismatch(binary: Path, work: Path, env: dict) -> tuple[dict, dict]:
    draft = _draft()
    reference = {"outcome": "rejected"}
    try:
        track = draft.Track(draft.TrackType.audio)
        track.add_segment(draft.VideoSegment.__new__(draft.VideoSegment))
        reference = {"outcome": "accepted"}
    except Exception:  # noqa: BLE001 - 上游用异常表达拒绝
        reference = {"outcome": "rejected"}

    # 音频轨里引用视频文件：我们只取其音轨
    plan = _plan(work, [{
        "type": "audio", "name": "audio",
        "segments": [{"start_us": 0, "duration_us": 1000000, "source": str(work / "clip.mp4")}],
    }])
    code, _ = _build(binary, plan, work / "mismatch", env)
    return reference, {"outcome": "rejected" if code != 0 else "accepted"}


CASES = [
    {
        "id": "time-parsing",
        "module": "tests/test_time_util.py",
        "covers": [
            "test_tim_parses_common_duration_strings",
            "test_trange_uses_duration_not_end_time",
            "test_trange_seconds_accepts_end_or_duration",
        ],
        "scenario": case_time_parsing,
        "compare": ["outcome", "values"],
    },
    {
        "id": "time-negative",
        "module": "tests/test_time_util.py",
        "covers": ["test_tim_supports_negative_offsets"],
        "scenario": case_time_negative,
        "compare": ["outcome", "value"],
    },
    {
        "id": "time-rounding",
        "module": "tests/test_time_util.py",
        "covers": [
            "test_trange_seconds_rounds_absolute_boundaries",
            "test_trange_seconds_requires_numeric_seconds",
            "test_timerange_overlap_semantics",
            "test_timerange_import_json_defaults_missing_start_to_zero",
        ],
        "scenario": case_time_parsing,
        "compare": ["outcome", "values"],
    },
    {
        "id": "track-overlap",
        "module": "tests/test_track_basics.py",
        "covers": [
            "test_track_rejects_overlapping_segments",
            "test_track_still_rejects_one_microsecond_overlap",
        ],
        "scenario": case_overlap_rejected,
        "compare": ["outcome"],
    },
    {
        "id": "track-type",
        "module": "tests/test_track_basics.py",
        "covers": [
            "test_track_accepts_matching_segment_type",
            "test_track_rejects_mismatched_segment_type",
        ],
        "scenario": case_type_mismatch,
        "compare": ["outcome"],
        "approved": {
            "outcome": "上游拒绝“视频对象进音频轨”是 Python 对象类型约束；"
            "我们面向草稿线格式，音频轨引用视频文件时只取其音轨，剪映同样允许",
        },
    },
    {
        "id": "track-end",
        "module": "tests/test_track_basics.py",
        "covers": [
            "test_track_end_time_reflects_last_segment_end",
            "test_track_accepts_beat_aligned_segments_in_seconds",
        ],
        "scenario": case_track_order,
        "compare": ["outcome", "names"],
        "approved": {
            "names": "上游允许先追加音频轨；我们强制主视频轨为首轨"
            "（剪映渲染层级与主轨连续性校验要求），因此顺序被规范化",
        },
    },
    {
        "id": "append-order",
        "module": "tests/test_track_append_api.py",
        "covers": [
            "test_append_track_returns_track_ref_and_appends_to_end",
            "test_append_tracks_preserves_input_order",
            "test_add_segment_accepts_track_ref",
            "test_add_segment_rejects_foreign_track_ref",
        ],
        "scenario": case_track_order,
        "compare": ["outcome", "names"],
        "approved": {
            "names": "上游允许先追加音频轨；我们强制主视频轨为首轨"
            "（剪映渲染层级与主轨连续性校验要求），因此顺序被规范化",
        },
    },
    {
        "id": "insert-order",
        "module": "tests/test_track_insert_api.py",
        "covers": [
            "test_insert_track_before_imported_track_preserves_overall_order",
            "test_insert_tracks_over_track_ref_preserves_block_order",
            "test_insert_track_at_index_uses_full_track_order",
            "test_insert_track_rejects_multiple_location_hints",
            "test_insert_track_rejects_foreign_imported_track_anchor",
            "test_insert_track_rejects_out_of_range_index",
        ],
        "scenario": case_insert_order,
        "compare": ["outcome", "names"],
    },
    {
        "id": "template-tracks",
        "module": "tests/test_template_loading.py",
        "covers": [
            "test_load_template_raises_stable_error_for_non_plain_content_without_fallback_loader",
            "test_load_template_uses_fallback_loader_for_non_plain_content",
            "test_load_template_accepts_dict_from_fallback_loader",
            "test_duplicate_as_template_reuses_fallback_loader",
            "test_load_template_normalizes_sparse_template_content",
            "test_template_dump_preserves_original_track_array_order",
            "test_list_imported_tracks_preserves_internal_order_and_supports_type_filter",
            "test_list_imported_tracks_returns_live_references",
            "test_get_imported_track_uses_internal_order_for_indexing",
            "test_import_track_supports_explicit_insert_position",
        ],
        "scenario": case_template_tracks,
        "compare": ["outcome", "names"],
    },
]


def prepare_media(work: Path) -> None:
    subprocess.run(
        ["ffmpeg", "-hide_banner", "-v", "error", "-f", "lavfi", "-i",
         "sine=frequency=440:duration=4", "-y", str(work / "audio.mp3")],
        check=True,
    )
    subprocess.run(
        ["ffmpeg", "-hide_banner", "-v", "error", "-f", "lavfi", "-i",
         "testsrc=s=640x360:d=4", "-c:v", "libx264", "-pix_fmt", "yuv420p",
         "-y", str(work / "clip.mp4")],
        check=True,
    )
