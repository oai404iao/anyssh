#!/usr/bin/env python3

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MARKDOWN_LINK = re.compile(r"\[[^\]]*\]\(([^)]+)\)")


def main() -> int:
    failures: list[tuple[Path, str]] = []
    checked = 0

    for document in sorted(ROOT.rglob("*.md")):
        if any(part in {"node_modules", "target", "artifacts"} for part in document.parts):
            continue

        checked += 1
        content = document.read_text(encoding="utf-8")

        for target in MARKDOWN_LINK.findall(content):
            if target.startswith(("http://", "https://", "mailto:", "#")):
                continue

            file_target = target.split("#", 1)[0]
            if not file_target:
                continue

            resolved = (document.parent / file_target).resolve()
            if not resolved.exists():
                failures.append((document.relative_to(ROOT), target))

    if failures:
        for document, target in failures:
            print(f"BROKEN {document}: {target}", file=sys.stderr)
        return 1

    print(f"Checked local Markdown links in {checked} files: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
