#!/usr/bin/env python3
"""Generate capability catalogs from GuanYixuan/pyJianYingDraft metadata.

The metadata (name/is_vip/resource_id/effect_id/md5/params tables) is
Apache-2.0 DATA from the upstream project; this script converts it into the
JSON catalogs jianying-cli embeds. Run against a pyJianYingDraft checkout:

    python3 tools/gen_catalogs.py /path/to/pyJianYingDraft
"""
import json, re, sys, hashlib
from pathlib import Path

DOMAINS = {
    "transitions":            ("transition_meta.py",      "TransitionMeta"),
    "filters":                ("filter_meta.py",          "EffectMeta"),
    "fonts":                  ("font_meta.py",            "EffectMeta"),
    "video_scene_effects":    ("video_scene_effect.py",   "EffectMeta"),
    "video_character_effects":("video_character_effect.py","EffectMeta"),
    "audio_scene_effects":    ("audio_scene_effect.py",   "EffectMeta"),
    "tone_effects":           ("tone_effect.py",          "EffectMeta"),
    "speech_to_songs":        ("speech_to_song.py",       "EffectMeta"),
    "video_animations_in":    ("video_intro.py",          "AnimationMeta"),
    "video_animations_out":   ("video_outro.py",          "AnimationMeta"),
    "video_animations_group": ("video_group_animation.py","AnimationMeta"),
    "text_animations_in":     ("text_intro.py",           "AnimationMeta"),
    "text_animations_out":    ("text_outro.py",           "AnimationMeta"),
    "text_animations_loop":   ("text_loop.py",            "AnimationMeta"),
    "masks":                  ("mask_meta.py",            "MaskMeta"),
    "mix_modes":              ("mix_mode_meta.py",        "EffectMeta"),
}

def split_args(s):
    args, depth, cur, q = [], 0, "", None
    for ch in s:
        if q:
            cur += ch
            if ch == q: q = None
            continue
        if ch in "\"'": q = ch; cur += ch; continue
        if ch in "([": depth += 1
        if ch in ")]": depth -= 1
        if ch == "," and depth == 0:
            args.append(cur.strip()); cur = ""
        else:
            cur += ch
    if cur.strip(): args.append(cur.strip())
    return args

def lit(tok):
    tok = tok.strip()
    if tok.startswith(("'", '"')):
        return tok[1:-1]
    return tok

PARAM_RE = re.compile(r'EffectParam\(\s*"((?:[^"\\]|\\.)*)"\s*,\s*([-\d.eE+]+)\s*,\s*([-\d.eE+]+)\s*,\s*([-\d.eE+]+)\s*\)')

def parse_member(meta_kind, argstr):
    args = split_args(argstr)
    name = lit(args[0])
    entry = {"name": name}
    rest = args[1:]
    if meta_kind == "TransitionMeta":
        entry.update(vip=lit(rest[0]) == "True", resource_id=lit(rest[1]),
                     effect_id=lit(rest[2]), md5=lit(rest[3]),
                     duration_us=round(float(lit(rest[4])) * 1_000_000),
                     is_overlap=lit(rest[5]) == "True")
    elif meta_kind == "AnimationMeta":
        entry.update(vip=lit(rest[0]) == "True",
                     duration_us=round(float(lit(rest[1])) * 1_000_000),
                     resource_id=lit(rest[2]), effect_id=lit(rest[3]), md5=lit(rest[4]))
    elif meta_kind == "MaskMeta":
        entry.update(shape=lit(rest[0]), resource_id=lit(rest[1]),
                     effect_id=lit(rest[2]), md5=lit(rest[3]),
                     default_aspect=float(lit(rest[4])))
    else:  # EffectMeta
        entry.update(vip=lit(rest[0]) == "True", resource_id=lit(rest[1]),
                     effect_id=lit(rest[2]), md5=lit(rest[3]))
        params_src = ",".join(rest[4:])
        entry["params"] = [{"name": n, "default": float(d), "min": float(lo), "max": float(hi)}
                           for n, d, lo, hi in PARAM_RE.findall(params_src)]
    return entry

MEMBER_RE = re.compile(r'^\s+\S+\s*=\s*(\w+Meta)\((.*)$', re.M)

def convert(meta_dir: Path, out_dir: Path, source_ref: str):
    index = {}
    for domain, (fname, meta_kind) in DOMAINS.items():
        src = meta_dir / fname
        text = src.read_text()
        matches = list(MEMBER_RE.finditer(text))
        entries = []
        for idx, m in enumerate(matches):
            start = m.start(2)
            end = matches[idx + 1].start() if idx + 1 < len(matches) else len(text)
            block = text[start:end]
            depth, close = 1, -1
            q = None
            for pos, ch in enumerate(block):
                if q:
                    if ch == q: q = None
                    continue
                if ch in "\"'": q = ch; continue
                if ch == "(": depth += 1
                elif ch == ")":
                    depth -= 1
                    if depth == 0: close = pos; break
            if close < 0: continue
            argstr = block[:close]
            if not argstr.strip(): continue
            try:
                entries.append(parse_member(meta_kind, argstr))
            except Exception as exc:
                print(f"WARN {domain}: skip member: {exc}", file=sys.stderr)
        out = out_dir / f"{domain}.json"
        out.write_text(json.dumps(entries, ensure_ascii=False, indent=1) + "\n")
        vip = sum(1 for e in entries if e.get("vip"))
        index[domain] = {"count": len(entries), "vip": vip,
                         "sha256": hashlib.sha256(out.read_bytes()).hexdigest()}
        print(f"{domain}: {len(entries)} entries ({vip} vip)")
    index_meta = {"source": "GuanYixuan/pyJianYingDraft (Apache-2.0)",
                  "source_ref": source_ref, "catalogs": index}
    (out_dir / "index.json").write_text(json.dumps(index_meta, ensure_ascii=False, indent=1) + "\n")

def main():
    import argparse
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("src", nargs="?", default="/tmp/pyjyd-study/pyJianYingDraft")
    ap.add_argument("--out", default=None, help="output catalogs dir (default: repo catalogs/)")
    ap.add_argument("--check", action="store_true",
                    help="verify the committed catalogs match the source metadata; exit 1 on drift")
    args = ap.parse_args()
    src = Path(args.src)
    meta_dir = src / "metadata"
    repo = Path(__file__).resolve().parents[1] / "catalogs"
    ref = subprocess_ref(src)
    if args.check:
        import tempfile
        with tempfile.TemporaryDirectory() as td:
            tmp = Path(td)
            convert(meta_dir, tmp, ref)
            drift = []
            for f in sorted(tmp.rglob("*.json")):
                committed = repo / f.name
                if not committed.is_file():
                    drift.append(f"+ {f.name} (missing in repo)")
                elif committed.read_text() != f.read_text():
                    drift.append(f"~ {f.name} (content differs)")
            for f in sorted(repo.glob("*.json")):
                if not (tmp / f.name).is_file():
                    drift.append(f"- {f.name} (stale in repo)")
            if drift:
                print("catalog drift vs pyJianYingDraft metadata:")
                for d in drift:
                    print(" ", d)
                sys.exit(1)
            print("catalogs up to date with", ref)
        return
    out_dir = Path(args.out) if args.out else repo
    out_dir.mkdir(exist_ok=True, parents=True)
    convert(meta_dir, out_dir, ref)

def subprocess_ref(src: Path) -> str:
    import subprocess
    try:
        return subprocess.run(["git", "-C", str(src), "log", "-1", "--format=%H %cI"],
                              capture_output=True, text=True, check=True).stdout.strip()
    except Exception:
        return "unknown"

if __name__ == "__main__":
    main()
