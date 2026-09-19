#!/usr/bin/env python3
"""Reference builder: construct a pyJianYingDraft draft from a
jianying-cli-plan/v1 file. This is the OTHER source of the dual-source
verification test suite — jianying-cli's output must match this semantically.

Usage: python3 tools/parity_reference.py plan.json out-dir [pyjyd-src]
"""
import json
import sys
from pathlib import Path

pyjyd_src = Path(sys.argv[3] if len(sys.argv) > 3 else "/tmp/pyjyd-study/pyJianYingDraft")
# sys.path needs the PARENT of the package directory
pkg_parent = pyjyd_src.parent if pyjyd_src.name == "pyJianYingDraft" else pyjyd_src
sys.path.insert(0, str(pkg_parent))

import os

import pyJianYingDraft as draft


def member(enum, name):
    for m in enum:
        n = getattr(m.value, "name", None) or getattr(m.value, "title", None)
        if n == name:
            return m
    raise KeyError(f"{name} not in {enum}")


def tim(v):
    if not isinstance(v, str):
        return v
    total = 0.0
    matched = False
    for num, unit in _re_findall_times(v):
        total += float(num) * {"h": 3.6e9, "m": 6e7, "s": 1e6, "ms": 1e3, "us": 1}[unit]
        matched = True
    return int(round(total)) if matched else v


def _re_findall_times(v):
    import re
    return re.findall("([0-9.]+)(h|ms|m|s|us)", v)


def preprocess_tim(node):
    if isinstance(node, dict):
        for k, val in list(node.items()):
            if isinstance(k, str) and k.endswith("_us") and isinstance(val, str):
                node[k] = tim(val)
            else:
                preprocess_tim(val)
    elif isinstance(node, list):
        for item in node:
            preprocess_tim(item)



def main():
    plan_path, out_dir = sys.argv[1], sys.argv[2]
    plan_dir = Path(plan_path).resolve().parent
    plan = json.loads(Path(plan_path).read_text())
    preprocess_tim(plan)
    def coerce(seg):
        for k in ("start_us", "duration_us", "source_start_us", "source_duration_us",
                  "at_us", "in_us", "out_us"):
            if isinstance(seg.get(k), str):
                seg[k] = tim(seg[k])
    for tr in plan.get("tracks", []):
        for sg in tr.get("segments", []):
            coerce(sg)
            for ch in (sg.get("keyframes") or {}).values():
                for pt in ch:
                    if isinstance(pt.get("at_us"), str):
                        pt["at_us"] = tim(pt["at_us"])
    for tr in plan.get("tracks", []):
        for sg in tr.get("segments", []):
            for k in ("start_us", "duration_us", "source_start_us", "source_duration_us"):
                if isinstance(sg.get(k), str):
                    sg[k] = tim(sg[k])
    store = Path(out_dir) / "ref"
    os.makedirs(store, exist_ok=True)
    canvas = plan["canvas"]
    script = draft.DraftFolder(str(store)).create_draft(
        plan["name"], canvas["width"], canvas["height"], fps=canvas["fps"])

    from pyJianYingDraft import (AudioSegment, TextSegment, Timerange, TrackSpec,
                                 TrackType, VideoSegment)

    for track in plan["tracks"]:
        kind = track["type"]
        tt = {"video": TrackType.video, "audio": TrackType.audio, "text": TrackType.text,
              "sticker": TrackType.sticker, "filter": TrackType.filter,
              "effect": TrackType.effect}[kind]
        ref = script.append_track(TrackSpec(tt))
        if kind == "filter":
            for seg in track["segments"]:
                f = seg["filters"][0]
                script.add_filter(member(draft.FilterType, f["name"]),
                                  Timerange(seg["start_us"], seg["duration_us"]),
                                  intensity=seg.get("intensity", 100.0))
            continue
        if kind == "effect":
            for seg in track["segments"]:
                e = seg["effects"][0]
                meta = (member(draft.VideoSceneEffectType, e["name"])
                        if any(getattr(x.value, "name", getattr(x.value, "title", None)) == e["name"]
                               for x in draft.VideoSceneEffectType)
                        else member(draft.VideoCharacterEffectType, e["name"]))
                script.add_effect(meta, Timerange(seg["start_us"], seg["duration_us"]))
            continue
        for seg in track["segments"]:
            if kind == "video":
                src = str((plan_dir / seg["source"]).resolve())
                speed = seg.get("speed", 1.0)
                material = draft.VideoMaterial(src)
                mat_dur = material.duration
                src_start = seg.get("source_start_us", 0)
                src_dur = seg.get("source_duration_us") or int(seg["duration_us"] * speed)
                vs = VideoSegment(
                    material,
                    target_timerange=Timerange(seg["start_us"], seg["duration_us"]),
                    source_timerange=Timerange(src_start, src_dur),
                    speed=speed,
                    volume=seg.get("volume", 1.0))
                if seg.get("transition_out"):
                    to = seg["transition_out"]
                    vs.add_transition(member(draft.TransitionType, to["name"]),
                                      duration=to.get("duration_us", 500_000))
                if seg.get("mask"):
                    mk = seg["mask"]
                    vs.add_mask(member(draft.MaskType, mk["name"]),
                                size=mk.get("size") or 0.5,
                                rotation=mk.get("rotation", 0.0),
                                feather=mk.get("feather", 0.0),
                                invert=mk.get("invert", False),
                                rect_width=mk.get("rect_width"),
                                round_corner=mk.get("round_corner"))
                for f in seg.get("filters", []):
                    vs.add_filter(member(draft.FilterType, f["name"]),
                                  intensity=f.get("intensity", 100.0))
                for e in seg.get("effects", []):
                    meta = member(draft.VideoSceneEffectType, e["name"])
                    params = ordered_params(meta, e.get("params", {}))
                    vs.add_effect(meta, params=params or None)
                if seg.get("mix_mode"):
                    vs.set_mix_mode(member(draft.MixModeType, seg["mix_mode"]))
                if seg.get("animation_in"):
                    vs.add_animation(member(draft.IntroType, seg["animation_in"]["name"]),
                                     duration=seg["animation_in"].get("duration_us"))
                if seg.get("animation_out"):
                    vs.add_animation(member(draft.OutroType, seg["animation_out"]["name"]),
                                     duration=seg["animation_out"].get("duration_us"))
                if seg.get("fade"):
                    vs.add_fade(seg["fade"]["in_us"], seg["fade"]["out_us"])
                if seg.get("chroma"):
                    c = seg["chroma"]
                    color8 = c["color"] + "FF" if len(c["color"].lstrip("#")) == 6 else c["color"]
                    vs.add_chroma(color8, intensity=c.get("intensity", 0.0),
                                  shadow=c.get("shadow", 0.0),
                                  edge_smooth=c.get("edge_smooth", 0.0),
                                  spill=c.get("spill", 0.0))
                if seg.get("background_filling"):
                    bf = seg["background_filling"]
                    vs.add_background_filling(bf["type"], blur=bf.get("blur", 0.0625),
                                              color=bf.get("color", "#00000000"))
                for channel, points in (seg.get("keyframes") or {}).items():
                    prop = {"scale": draft.KeyframeProperty.scale_x,
                            "x": draft.KeyframeProperty.position_x,
                            "y": draft.KeyframeProperty.position_y,
                            "rotation": draft.KeyframeProperty.rotation,
                            "opacity": draft.KeyframeProperty.alpha}[channel]
                    for p in points:
                        vs.add_keyframe(prop, p["at_us"], p["value"])
                script.add_segment(vs, ref)
            elif kind == "audio":
                src = str((plan_dir / seg["source"]).resolve())
                speed = seg.get("speed", 1.0)
                src_dur = seg.get("source_duration_us") or int(seg["duration_us"] * speed)
                aus = AudioSegment(
                    src,
                    target_timerange=Timerange(seg["start_us"], seg["duration_us"]),
                    source_timerange=Timerange(seg.get("source_start_us", 0), src_dur),
                    speed=speed,
                    volume=seg.get("volume", 1.0))
                if seg.get("fade"):
                    aus.add_fade(seg["fade"]["in_us"], seg["fade"]["out_us"])
                for e in seg.get("audio_effects", []):
                    for pool in (draft.AudioSceneEffectType, draft.ToneEffectType,
                                 draft.SpeechToSongType):
                        try:
                            meta = member(pool, e["name"])
                            aus.add_effect(meta, params=ordered_params(meta, e.get("params", {})) or None)
                            break
                        except KeyError:
                            continue
                script.add_segment(aus, ref)
            elif kind == "text":
                from pyJianYingDraft import ClipSettings, TextBackground, TextBorder, TextShadow, TextStyle
                style = TextStyle(
                    size=seg.get("size", 8.0),
                    bold=seg.get("bold", False),
                    italic=seg.get("italic", False),
                    underline=seg.get("underline", False),
                    color=tuple(draft_text_color(seg.get("color", "#FFFFFF"))),
                    align=seg.get("alignment", 1))
                kwargs = {}
                bw = seg.get("border_width")
                if bw:
                    kwargs["border"] = TextBorder(
                        color=tuple(draft_text_color(seg.get("border_color", "#000000"))),
                        width=bw)
                if seg.get("shadow"):
                    sh = seg["shadow"]
                    kwargs["shadow"] = TextShadow(
                        color=tuple(draft_text_color(sh.get("color", "#000000"))),
                        alpha=sh.get("alpha", 1.0),
                        angle=sh.get("angle", -45.0),
                        distance=sh.get("distance", 5.0),
                        diffuse=sh.get("diffuse", 15.0))
                if seg.get("background"):
                    bg = seg["background"]
                    kwargs["background"] = TextBackground(
                        color=bg["color"], style=bg.get("style", 1),
                        alpha=bg.get("alpha", 1.0),
                        round_radius=bg.get("round_radius", 0.0),
                        height=bg.get("height", 0.14), width=bg.get("width", 0.14),
                        horizontal_offset=bg.get("horizontal_offset", 0.5),
                        vertical_offset=bg.get("vertical_offset", 0.5))
                txt = TextSegment(
                    seg["text"], timerange=Timerange(seg["start_us"], seg["duration_us"]),
                    style=style, clip_settings=ClipSettings(transform_y=seg.get("y", -0.78)),
                    **kwargs)
                if seg.get("animation_in"):
                    txt.add_animation(member(draft.TextIntro, seg["animation_in"]["name"]),
                                      duration=seg["animation_in"].get("duration_us"))
                script.add_segment(txt, ref)
    script.dump(str(store / plan["name"] / "draft_content.json"))
    print("reference built:", plan["name"])


def ordered_params(meta, user_params):
    """Convert a name→0-100 map into pyJianYingDraft's positional list."""
    names = [p.name if hasattr(p, "name") else (p["name"] if isinstance(p, dict) else None)
             for p in getattr(meta.value, "params", [])]
    if not user_params:
        return []
    return [user_params.get(n) for n in names]


def draft_text_color(hex_str):
    h = hex_str.lstrip("#")
    return tuple(int(h[i * 2:i * 2 + 2], 16) / 255.0 for i in range(3))


if __name__ == "__main__":
    main()
