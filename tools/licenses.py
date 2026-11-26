#!/usr/bin/env python3
"""Dependency licence check for the shipped client (AGPL-3.0 + app store
permission, see LICENSE-EXCEPTIONS.md).

Every package that ends up in the app must have a licence on the allow-list:
licences compatible with AGPL-3.0 that add no app-store conflict of their own.
Copyleft licences that could conflict (GPL, AGPL, LGPL) are only allowed for
named packages after review (REVIEWED). Unknown or missing licences fail.

Usage:
  tools/licenses.py                 # check the app (Rust + npm runtime deps)
  tools/licenses.py --report FILE   # also write THIRD_PARTY_NOTICES.md
  tools/licenses.py --self-test     # check the checker
"""
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# SPDX ids that are fine in an AGPL app shipped through app stores.
ALLOWED = {
    "MIT", "MIT-0", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause",
    "BSD-3-Clause", "ISC", "Zlib", "0BSD", "Unlicense", "CC0-1.0", "BSL-1.0",
    "Unicode-3.0", "Unicode-DFS-2016", "MPL-2.0", "CDLA-Permissive-2.0", "bzip2-1.0.6",
}

# Copyleft packages allowed after a review (name -> reason).
REVIEWED = {
    "verovio": "LGPL-3.0: whole app source is public under AGPL, so relinking is possible; lawyer check in phase L",
    # Android only (Gradle, not scanned by this tool; listed for the record).
    "com.google.android.gms:play-services-mlkit-document-scanner": "ML Kit Terms of Service (proprietary, free): LIB-12 camera scan on Android, runs in Google Play services on the device; accepted by the product owner 2026-11-26 (option a, like homr)",
}

# Our own crates (AGPL-3.0-only, covered by LICENSE-EXCEPTIONS.md).
OURS = re.compile(r"^cantor(-|$)")


def spdx_ok(expr: str) -> bool:
    """True if an SPDX expression is satisfiable with allowed licences only.
    Handles OR / AND / parentheses and the old 'MIT/Apache-2.0' form."""
    if not expr:
        return False
    expr = expr.replace("/", " OR ").strip()
    tokens = re.findall(r"\(|\)|\bAND\b|\bOR\b|[^\s()]+(?:\s+WITH\s+[^\s()]+)?", expr)
    pos = 0

    def parse_or():
        nonlocal pos
        v = parse_and()
        while pos < len(tokens) and tokens[pos] == "OR":
            pos += 1
            v = parse_and() or v
        return v

    def parse_and():
        nonlocal pos
        v = parse_atom()
        while pos < len(tokens) and tokens[pos] == "AND":
            pos += 1
            v = parse_atom() and v
        return v

    def parse_atom():
        nonlocal pos
        if pos >= len(tokens):
            return False
        t = tokens[pos]
        pos += 1
        if t == "(":
            v = parse_or()
            pos += 1  # ')'
            return v
        return t in ALLOWED

    try:
        ok = parse_or()
        return ok and pos == len(tokens)
    except IndexError:
        return False


def cargo_packages(manifest):
    """Packages in the normal (non-dev, non-build) dependency tree of the app
    for iOS and macOS."""
    out = []
    for target in ("aarch64-apple-ios", "aarch64-apple-darwin"):
        meta = json.loads(subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--manifest-path", manifest,
             "--filter-platform", target],
            cwd=ROOT,
        ))
        pkgs = {p["id"]: p for p in meta["packages"]}
        nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
        seen = set()
        stack = [meta["resolve"]["root"]] if meta["resolve"].get("root") else list(meta["workspace_members"])
        while stack:
            i = stack.pop()
            if i in seen:
                continue
            seen.add(i)
            for d in nodes[i]["deps"]:
                if any(k["kind"] in (None, "normal") for k in d["dep_kinds"]):
                    stack.append(d["pkg"])
        for i in seen:
            p = pkgs[i]
            out.append((p["name"], p["version"], p.get("license") or "", "crates.io" if p.get("source") else "local"))
    return sorted(set(out))


def npm_packages(ui):
    """Runtime npm dependencies (what is bundled into the app)."""
    out = []
    try:
        tree = json.loads(subprocess.check_output(
            ["npm", "ls", "--omit=dev", "--all", "--json"], cwd=ui, stderr=subprocess.DEVNULL))
    except subprocess.CalledProcessError as e:
        tree = json.loads(e.output)

    def walk(deps):
        for name, d in (deps or {}).items():
            # Optional platform packages that are not installed (e.g.
            # @napi-rs/canvas for other OSes, used by pdf.js under Node only)
            # cannot end up in the app bundle.
            if d.get("missing") or "version" not in d:
                continue
            path = os.path.join(ui, "node_modules", name, "package.json")
            lic = ""
            if os.path.exists(path):
                j = json.load(open(path))
                lic = j.get("license") or ""
                if isinstance(lic, dict):
                    lic = lic.get("type", "")
            out.append((name, d.get("version", "?"), lic, "npm"))
            walk(d.get("dependencies"))

    walk(tree.get("dependencies"))
    return sorted(set(out))


def check(packages):
    bad = []
    for name, ver, lic, src in packages:
        if OURS.match(name):
            continue
        if name in REVIEWED or spdx_ok(lic):
            continue
        bad.append((name, ver, lic or "(none)", src))
    return bad


def notices(packages):
    lines = [
        "# Third-party notices",
        "",
        "Cantor is AGPL-3.0 (see `LICENSE` and `LICENSE-EXCEPTIONS.md`). It includes",
        "the following third-party components under their own licences. Generated by",
        "`tools/licenses.py --report`; do not edit by hand.",
        "",
        "Bundled assets: GeneralUser GS soundfont (S. Christian Collins, free for",
        "commercial use, `assets/sf2/GeneralUser-GS.LICENSE.txt`); pdf.js (Apache-2.0);",
        "Verovio (LGPL-3.0, https://github.com/rism-digital/verovio).",
        "",
        "Builds with on-device note recognition (`homr` feature) also contain",
        "`crates/homr`, a Rust port of homr 0.7.1 (Christian Liebhardt and the homr",
        "contributors, AGPL-3.0, https://github.com/liebharc/homr), and homr's model",
        "weights (AGPL-3.0). The segmentation model is trained on DeepScoresV2",
        "(Tuggener et al., CC BY 4.0, https://zenodo.org/records/4012193); the",
        "transformer on OpenScore Lieder and StringQuartets (CC0), GrandStaff (MIT)",
        "and the public-domain subset of PDMX (CC BY 4.0,",
        "https://zenodo.org/records/15571083). ONNX Runtime (Microsoft, MIT) runs them.",
        "",
        "Android builds use Google's ML Kit document scanner",
        "(`com.google.android.gms:play-services-mlkit-document-scanner`, ML Kit Terms",
        "of Service, https://developers.google.com/ml-kit/terms) for scanning sheet",
        "music with the camera. It runs on the device in Google Play services.",
        "",
        "| Package | Version | Licence | Source |",
        "|---|---|---|---|",
    ]
    for name, ver, lic, src in packages:
        if OURS.match(name):
            continue
        lines.append(f"| {name} | {ver} | {lic or '(none)'} | {src} |")
    return "\n".join(lines) + "\n"


def self_test():
    assert spdx_ok("MIT")
    assert spdx_ok("MIT OR Apache-2.0")
    assert spdx_ok("MIT/Apache-2.0")
    assert spdx_ok("(MIT OR Apache-2.0) AND Unicode-3.0")
    assert spdx_ok("Apache-2.0 WITH LLVM-exception")
    assert spdx_ok("GPL-3.0 OR MIT"), "an OR with an allowed choice is fine"
    assert not spdx_ok("GPL-3.0-only")
    assert not spdx_ok("AGPL-3.0-only")
    assert not spdx_ok("MIT AND GPL-2.0")
    assert not spdx_ok("")
    assert not spdx_ok("Proprietary")
    bad = check([("x", "1", "GPL-3.0", "npm"), ("cantor-score", "0.1", "AGPL-3.0-only", "local"),
                 ("verovio", "6", "LGPL-3.0-only", "npm"), ("y", "1", "", "crates.io"), ("z", "1", "MIT", "npm")])
    assert [b[0] for b in bad] == ["x", "y"], bad
    print("self-test ok")


def main():
    if "--self-test" in sys.argv:
        self_test()
        return 0
    pkgs = cargo_packages(os.path.join(ROOT, "app/src-tauri/Cargo.toml"))
    pkgs += npm_packages(os.path.join(ROOT, "app/ui"))
    bad = check(pkgs)
    if "--report" in sys.argv:
        path = sys.argv[sys.argv.index("--report") + 1]
        open(path, "w").write(notices(pkgs))
    print(f"{len(pkgs)} third-party packages checked, {len(bad)} not allowed")
    for b in bad:
        print("  NOT ALLOWED:", *b)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
