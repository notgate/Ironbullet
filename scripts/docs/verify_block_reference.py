#!/usr/bin/env python3
"""Fail CI when a native BlockType has no block-reference entry, or vice versa."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MODEL = ROOT / "src/pipeline/block/mod.rs"
REFERENCE = ROOT / "docs/site/src/content/docs/blocks"


def block_types() -> set[str]:
    source = MODEL.read_text(encoding="utf-8")
    enum_body = source.split("pub enum BlockType {", 1)[1].split("}", 1)[0]
    return set(re.findall(r"^\s{4}([A-Z][A-Za-z0-9]+),", enum_body, re.MULTILINE))


def documented_types() -> set[str]:
    entries: set[str] = set()
    for page in REFERENCE.glob("*.md"):
        entries.update(re.findall(r"\*\*Rust type:\*\* `([A-Za-z0-9]+)`", page.read_text(encoding="utf-8")))
    return entries


def main() -> int:
    native = block_types()
    documented = documented_types()
    missing = sorted(native - documented)
    extra = sorted(documented - native)
    if missing or extra:
        if missing:
            print("Missing block documentation: " + ", ".join(missing), file=sys.stderr)
        if extra:
            print("Stale block documentation: " + ", ".join(extra), file=sys.stderr)
        return 1
    print(f"Block-reference coverage verified: {len(native)} native BlockType entries.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
