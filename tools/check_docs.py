#!/usr/bin/env python3
"""Check that the docs point at things that exist: markdown links to files,
and backticked repo paths (crates/..., tools/..., docs/..., app/...).
Exit 1 on any broken reference. Run from anywhere."""
import os, re, sys, urllib.parse

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOCS = ["README.md", "AGENTS.md", "CLAUDE.md", "CONTRIBUTING.md", "SECURITY.md", "LICENSE-EXCEPTIONS.md",
        "docs/README.md", "docs/ARCHITECTURE.md", "docs/DEVELOPING.md", "docs/ROADMAP.md",
        "docs/monetization-feasibility.md", "tools/omr/spike/README.md"]
PATH_RE = re.compile(r"`((?:crates|tools|docs|app|fixtures|assets|\.github)/[A-Za-z0-9_./*<>-]+)`")
LINK_RE = re.compile(r"\]\(([^)#]+)(?:#[^)]*)?\)")
# Paths that are meant as patterns or that only exist on a built/dev machine.
SKIP = ("<", "*", "app/src-tauri/resources/scores", "app/src-tauri/gen/apple/build", "app/ui/dist",
        "app/src-tauri/gen/apple/assets")

bad = []
for doc in DOCS:
    path = os.path.join(ROOT, doc)
    text = open(path, encoding="utf-8").read()
    base = os.path.dirname(path)
    for m in LINK_RE.finditer(text):
        target = urllib.parse.unquote(m.group(1))
        if target.startswith(("http://", "https://", "mailto:")):
            continue
        if not os.path.exists(os.path.normpath(os.path.join(base, target))):
            bad.append((doc, "link", target))
    for m in PATH_RE.finditer(text):
        p = m.group(1).rstrip("/.")
        if any(s in p for s in SKIP):
            continue
        if not os.path.exists(os.path.join(ROOT, p)):
            bad.append((doc, "path", p))
for b in bad:
    print("BROKEN", *b)
print(f"{len(DOCS)} docs checked, {len(bad)} broken references")
sys.exit(1 if bad else 0)
