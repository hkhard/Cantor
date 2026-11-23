#!/usr/bin/env python3
"""Compare OMR engine output (homr, oemer, ...) against Audiveris MusicXML.

Audiveris is a reference here, NOT ground truth. The numbers say how much the
engines agree with what Cantor currently gets from Audiveris.

Layout expected (produced by a per-page benchmark run):

    <run-dir>/<engine>/<stem>-p<N>/<stem>-p<N>.musicxml   engine output per page
    <run-dir>/<engine>/<stem>-p<N>.time                   `/usr/bin/time -l` stderr,
                                                          plus a final `exit=<code>` line
    <ref-dir>/<stem>.mxl | <stem>.musicxml                Audiveris output per score

Method, per score:
  * Every staff of every part becomes one "stream": the notes on that staff in
    reading order (measure, onset; chord notes high to low), as MIDI pitches.
    Rests and grace notes are skipped. Engine pages are concatenated in page
    order, stream i of page k followed by stream i of page k+1.
  * Engine streams are matched to reference streams greedily by best agreement
    (engines split voices into parts/staves differently, so index order is not
    reliable).
  * agreement = 1 - levenshtein(a, b) / max(len a, len b), per matched pair,
    then averaged weighted by reference stream length. Also reported with
    pitches folded to pitch class (octave errors, e.g. a missed tenor 8vb clef,
    do not count) and a pitch-histogram overlap (sum of min over MIDI bins /
    max of the two totals), which ignores order and stream assignment.

Usage:
    python tools/omr/compare_engines.py --run-dir RUN --ref-dir scores/out \
        --engines homr oemer --json out.json --markdown out.md

Only the Python standard library and numpy are needed.
"""
from __future__ import annotations

import argparse
import glob
import json
import os
import re
import statistics
import sys
import xml.etree.ElementTree as ET
import zipfile
from fractions import Fraction

import numpy as np

STEP = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}


# ---------------------------------------------------------------- MusicXML ---

def load_xml(path: str) -> ET.Element:
    if path.endswith(".mxl"):
        with zipfile.ZipFile(path) as z:
            root_file = None
            if "META-INF/container.xml" in z.namelist():
                c = ET.fromstring(z.read("META-INF/container.xml"))
                rf = c.find(".//{*}rootfile")
                if rf is not None:
                    root_file = rf.get("full-path")
            if root_file is None:
                root_file = next(n for n in z.namelist()
                                 if n.endswith((".xml", ".musicxml")) and not n.startswith("META-INF"))
            data = z.read(root_file)
    else:
        with open(path, "rb") as f:
            data = f.read()
    root = ET.fromstring(data)
    for el in root.iter():  # strip namespaces if any
        if isinstance(el.tag, str) and "}" in el.tag:
            el.tag = el.tag.split("}", 1)[1]
    return root


def parse_score(path: str) -> dict:
    """Return {'measures': int (max over parts, non-empty only),
               'measures_raw': int,
               'streams': {(part_idx, staff): [(measure_idx, onset, midi), ...]}}"""
    root = load_xml(path)
    streams: dict[tuple[int, int], list] = {}
    measures_nonempty = 0
    measures_raw = 0
    for pi, part in enumerate(root.findall("part")):
        divisions = 1
        nonempty = 0
        ms = part.findall("measure")
        measures_raw = max(measures_raw, len(ms))
        for mi, meas in enumerate(ms):
            pos = Fraction(0)
            last_start = Fraction(0)
            has_event = False
            for el in meas:
                if el.tag == "attributes":
                    d = el.find("divisions")
                    if d is not None and d.text:
                        divisions = int(float(d.text))
                elif el.tag == "backup":
                    pos -= Fraction(int(float(el.findtext("duration", "0"))), divisions)
                elif el.tag == "forward":
                    pos += Fraction(int(float(el.findtext("duration", "0"))), divisions)
                elif el.tag == "note":
                    has_event = True
                    if el.find("grace") is not None:
                        continue
                    dur = Fraction(int(float(el.findtext("duration", "0") or 0)), divisions)
                    if el.find("chord") is not None:
                        start = last_start
                    else:
                        start = pos
                        pos += dur
                        last_start = start
                    p = el.find("pitch")
                    if p is None or el.find("rest") is not None:
                        continue
                    step = p.findtext("step", "C").strip()
                    alter = float(p.findtext("alter", "0") or 0)
                    octave = int(p.findtext("octave", "4"))
                    midi = 12 * (octave + 1) + STEP.get(step, 0) + int(round(alter))
                    staff = int(el.findtext("staff", "1") or 1)
                    streams.setdefault((pi, staff), []).append((mi, start, midi))
            if has_event:
                nonempty += 1
        measures_nonempty = max(measures_nonempty, nonempty)
    for k, v in streams.items():
        v.sort(key=lambda t: (t[0], t[1], -t[2]))
    return {"measures": measures_nonempty, "measures_raw": measures_raw, "streams": streams}


def ordered_pitch_streams(score: dict) -> list[list[int]]:
    return [[m for (_, _, m) in score["streams"][k]] for k in sorted(score["streams"])]


# ----------------------------------------------------------------- metrics ---

def levenshtein(a: list[int], b: list[int]) -> int:
    if not a:
        return len(b)
    if not b:
        return len(a)
    # Row-vectorised DP. Without insertions, row i is
    # best[j] = min(prev[j-1] + (a[i] != b[j]), prev[j] + 1), with best[0] = i.
    # Insertions give cur[j] = min_k<=j (best[k] + j - k), a prefix minimum of best[k] - k.
    av = np.asarray(a, dtype=np.int32)
    bv = np.asarray(b, dtype=np.int32)
    idx = np.arange(len(bv) + 1, dtype=np.int32)
    prev = idx.copy()
    for i in range(1, len(av) + 1):
        best = np.empty_like(prev)
        best[0] = i
        best[1:] = np.minimum(prev[:-1] + (bv != av[i - 1]), prev[1:] + 1)
        prev = np.minimum.accumulate(best - idx) + idx
    return int(prev[-1])


def agreement(a: list[int], b: list[int]) -> float:
    n = max(len(a), len(b))
    if n == 0:
        return 1.0
    return 1.0 - levenshtein(a, b) / n


def hist_overlap(a: list[int], b: list[int]) -> float:
    if not a and not b:
        return 1.0
    ha = np.bincount(a, minlength=128) if a else np.zeros(128, int)
    hb = np.bincount(b, minlength=128) if b else np.zeros(128, int)
    n = max(len(ha), len(hb))
    ha = np.pad(ha, (0, n - len(ha)))
    hb = np.pad(hb, (0, n - len(hb)))
    return float(np.minimum(ha, hb).sum() / max(ha.sum(), hb.sum()))


def match_streams(ref: list[list[int]], cand: list[list[int]], fold: bool,
                  octave_shift: bool = False) -> tuple[float, list]:
    """Greedy one-to-one matching ref<->cand by agreement. Returns
    (agreement weighted by reference length, pairs).

    fold: compare pitch classes only.
    octave_shift: allow shifting a whole candidate stream by -12/+12 semitones
    (catches a consistently misread octave, e.g. an ignored tenor "8vb" clef,
    while still counting individual wrong notes)."""
    f = (lambda s: [x % 12 for x in s]) if fold else (lambda s: s)
    R = [f(s) for s in ref]
    C = [f(s) for s in cand]
    shifts = (0, -12, 12) if (octave_shift and not fold) else (0,)
    scores = []
    for i, r in enumerate(R):
        for j, c in enumerate(C):
            best = max(agreement(r, [x + k for x in c]) for k in shifts)
            scores.append((best, i, j))
    scores.sort(reverse=True)
    used_r, used_c, pairs = set(), set(), []
    for s, i, j in scores:
        if i in used_r or j in used_c:
            continue
        used_r.add(i)
        used_c.add(j)
        pairs.append((i, j, s))
    total = sum(len(r) for r in ref) or 1
    weighted = sum(len(ref[i]) * s for i, _, s in pairs) / total
    # unmatched reference streams count as 0 agreement (already excluded from sum)
    return weighted, pairs


# ------------------------------------------------------------- run records ---

def parse_time(path: str) -> dict:
    out = {"wall_s": None, "peak_rss_mb": None, "peak_footprint_mb": None, "exit": None}
    if not os.path.exists(path):
        return out
    txt = open(path, errors="replace").read()
    m = re.search(r"([\d.]+) real", txt)
    if m:
        out["wall_s"] = float(m.group(1))
    m = re.search(r"(\d+)\s+maximum resident set size", txt)
    if m:
        out["peak_rss_mb"] = int(m.group(1)) / 1e6  # macOS reports bytes
    m = re.search(r"(\d+)\s+peak memory footprint", txt)
    if m:
        out["peak_footprint_mb"] = int(m.group(1)) / 1e6
    m = re.search(r"exit=(\d+)", txt)
    if m:
        out["exit"] = int(m.group(1))
    return out


def page_key(stem_page: str) -> tuple[str, int]:
    m = re.match(r"(.*)-p(\d+)$", stem_page)
    return (m.group(1), int(m.group(2))) if m else (stem_page, 0)


def find_reference(ref_dir: str, stem: str) -> str | None:
    cands = [p for p in (os.path.join(ref_dir, stem + ".mxl"), os.path.join(ref_dir, stem + ".musicxml"))
             if os.path.exists(p)]
    if not cands:
        return None
    if len(cands) == 1:
        return cands[0]
    # Two Audiveris exports for one stem: use the one with more measures (the complete one).
    return max(cands, key=lambda p: parse_score(p)["measures"])


def restrict_to_pages(score: dict, measure_pages: list[int], keep: set[int]) -> dict:
    """Keep only reference notes in measures that Audiveris placed on pages in `keep`.
    measure_pages[i] is the PDF page of measure i (from <stem>.measures.json)."""
    ok = {i for i, p in enumerate(measure_pages) if p in keep}
    streams = {k: [t for t in v if t[0] in ok] for k, v in score["streams"].items()}
    return {"measures": len(ok), "measures_raw": len(ok),
            "streams": {k: v for k, v in streams.items() if v}}


def collect(run_dir: str, ref_dir: str, engines: list[str], coverage_dir: str | None = None,
            first_page_only: bool = False) -> dict:
    pages: dict[str, dict[str, dict]] = {}
    for eng in engines:
        for tf in sorted(glob.glob(os.path.join(run_dir, eng, "*.time"))):
            sp = os.path.basename(tf)[:-5]
            rec = parse_time(tf)
            xmls = glob.glob(os.path.join(run_dir, eng, sp, "*.musicxml"))
            rec["xml"] = xmls[0] if xmls else None
            rec["ok"] = rec["exit"] == 0 and rec["xml"] is not None
            if rec["xml"]:
                try:
                    rec["parsed"] = parse_score(rec["xml"])
                except ET.ParseError as e:
                    rec["ok"] = False
                    rec["error"] = f"bad XML: {e}"
            pages.setdefault(sp, {})[eng] = rec

    scores: dict[str, dict] = {}
    for sp in pages:
        stem, n = page_key(sp)
        scores.setdefault(stem, {"pages": []})["pages"].append((n, sp))

    result = {"scores": {}, "pages": {}}
    for sp, d in pages.items():
        result["pages"][sp] = {e: {k: v for k, v in r.items() if k != "parsed"} for e, r in d.items()}

    for stem in sorted(scores):
        plist = [sp for _, sp in sorted(scores[stem]["pages"])]
        all_pages = len(plist)
        covered = None
        measure_pages = None
        if coverage_dir:
            cj = os.path.join(coverage_dir, stem + ".measures.json")
            if os.path.exists(cj):
                with open(cj) as f:
                    measure_pages = [int(m["page"]) for m in json.load(f)]
                covered = sorted(set(measure_pages))
                plist = [sp for sp in plist if page_key(sp)[1] in covered]
        if first_page_only:
            plist = plist[:1]
        ref_path = find_reference(ref_dir, stem)
        ref = parse_score(ref_path) if ref_path else None
        if ref and first_page_only and measure_pages and plist:
            ref = restrict_to_pages(ref, measure_pages, {page_key(plist[0])[1]})
        ref_streams = ordered_pitch_streams(ref) if ref else []
        entry = {"pages": len(plist), "pages_in_pdf": all_pages, "covered_pages": covered,
                 "reference": os.path.basename(ref_path) if ref_path else None}
        if ref:
            entry["audiveris"] = {
                "measures": ref["measures"],
                "notes_per_stream": [len(s) for s in ref_streams],
                "notes": sum(len(s) for s in ref_streams),
            }
        for eng in engines:
            recs = [pages[sp].get(eng) for sp in plist]
            ok = [r for r in recs if r and r.get("ok") and r.get("parsed")]
            times = [r["wall_s"] for r in recs if r and r.get("wall_s") is not None]
            rss = [r["peak_rss_mb"] for r in recs if r and r.get("peak_rss_mb") is not None]
            e = {
                "pages_ok": len(ok),
                "pages_failed": [sp for sp, r in zip(plist, recs) if not (r and r.get("ok"))],
                "s_per_page": statistics.mean(times) if times else None,
                "peak_rss_mb": max(rss) if rss else None,
            }
            # concatenate pages stream-by-index
            concat: list[list[int]] = []
            measures = 0
            for r in recs:
                if not (r and r.get("parsed")):
                    continue
                measures += r["parsed"]["measures"]
                for i, s in enumerate(ordered_pitch_streams(r["parsed"])):
                    while len(concat) <= i:
                        concat.append([])
                    concat[i].extend(s)
            e["measures"] = measures
            e["notes_per_stream"] = [len(s) for s in concat]
            e["notes"] = sum(len(s) for s in concat)
            if ref:
                agr, pairs = match_streams(ref_streams, concat, fold=False)
                agr_pc, _ = match_streams(ref_streams, concat, fold=True)
                agr_oct, _ = match_streams(ref_streams, concat, fold=False, octave_shift=True)
                flat_r = [x for s in ref_streams for x in s]
                flat_c = [x for s in concat for x in s]
                e["agreement"] = agr
                e["agreement_pitch_class"] = agr_pc
                e["agreement_octave_shift"] = agr_oct
                e["hist_overlap"] = hist_overlap(flat_r, flat_c)
                e["stream_pairs"] = [(i, j, round(s, 3)) for i, j, s in pairs]
            entry[eng] = e
        result["scores"][stem] = entry
    return result


def fmt(x, spec="{:.2f}"):
    return "n/a" if x is None else spec.format(x)


def markdown(result: dict, engines: list[str]) -> str:
    lines = ["| Score | Pages | Engine | OK pages | Measures | Notes (per staff) | Agreement | Agr. (octave-shift ok) | Agr. (pitch class) | Histogram overlap | s/page | Peak RSS MB |",
             "|---|---|---|---|---|---|---|---|---|---|---|---|"]
    for stem, e in result["scores"].items():
        a = e.get("audiveris")
        if a:
            lines.append(f"| {stem} | {e['pages']} | Audiveris (ref) | | {a['measures']} | {a['notes']} ({', '.join(map(str, a['notes_per_stream']))}) | | | | | | |")
        else:
            lines.append(f"| {stem} | {e['pages']} | Audiveris (ref) | | no reference | | | | | | | |")
        for eng in engines:
            r = e.get(eng)
            if not r:
                continue
            per = ", ".join(map(str, r["notes_per_stream"]))
            lines.append(
                f"| | | {eng} | {r['pages_ok']}/{e['pages']} | {r['measures']} | {r['notes']} ({per}) | "
                f"{fmt(r.get('agreement'), '{:.0%}')} | {fmt(r.get('agreement_octave_shift'), '{:.0%}')} | "
                f"{fmt(r.get('agreement_pitch_class'), '{:.0%}')} | "
                f"{fmt(r.get('hist_overlap'), '{:.0%}')} | {fmt(r['s_per_page'], '{:.1f}')} | {fmt(r['peak_rss_mb'], '{:.0f}')} |")
    return "\n".join(lines) + "\n"


def summary(result: dict, engines: list[str]) -> dict:
    out = {}
    for eng in engines:
        rows = [e[eng] for e in result["scores"].values() if eng in e]
        pages = [p[eng] for p in result["pages"].values() if eng in p]
        wall = [p["wall_s"] for p in pages if p.get("wall_s") is not None]
        rss = [p["peak_rss_mb"] for p in pages if p.get("peak_rss_mb") is not None]
        refd = [r for r, e in zip(rows, result["scores"].values()) if e.get("audiveris")]
        wsum = sum(e["audiveris"]["notes"] for e in result["scores"].values() if e.get("audiveris") and eng in e) or 1
        out[eng] = {
            "pages": len(pages),
            "pages_ok": sum(1 for p in pages if p.get("ok")),
            "median_s_per_page": statistics.median(wall) if wall else None,
            "max_rss_mb": max(rss) if rss else None,
            "median_rss_mb": statistics.median(rss) if rss else None,
            "mean_agreement": statistics.mean([r["agreement"] for r in refd]) if refd else None,
            "note_weighted_agreement": sum(r["agreement"] * e["audiveris"]["notes"]
                                           for r, e in zip(rows, result["scores"].values())
                                           if e.get("audiveris") and "agreement" in r) / wsum,
            "mean_agreement_octave_shift": statistics.mean([r["agreement_octave_shift"] for r in refd]) if refd else None,
            "mean_agreement_pitch_class": statistics.mean([r["agreement_pitch_class"] for r in refd]) if refd else None,
            "mean_hist_overlap": statistics.mean([r["hist_overlap"] for r in refd]) if refd else None,
        }
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--run-dir", required=True)
    ap.add_argument("--ref-dir", required=True)
    ap.add_argument("--engines", nargs="+", default=["homr", "oemer"])
    ap.add_argument("--coverage-dir", help="directory with <stem>.measures.json (Audiveris measure boxes); "
                    "when given, only pages Audiveris recognised are compared")
    ap.add_argument("--first-page-only", action="store_true",
                    help="compare only the first (covered) page of each score; needs --coverage-dir to cut "
                    "the Audiveris reference down to that page's measures")
    ap.add_argument("--json")
    ap.add_argument("--markdown")
    args = ap.parse_args()
    res = collect(args.run_dir, args.ref_dir, args.engines, args.coverage_dir, args.first_page_only)
    res["summary"] = summary(res, args.engines)
    md = markdown(res, args.engines)
    if args.json:
        with open(args.json, "w") as f:
            json.dump(res, f, indent=1, default=str)
    if args.markdown:
        with open(args.markdown, "w") as f:
            f.write(md)
    print(md)
    print(json.dumps(res["summary"], indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
