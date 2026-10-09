#!/usr/bin/env python3
"""Deterministic 5MB Markdown fixture; does not contain external attachments."""
from pathlib import Path
import sys

vault = Path(sys.argv[1])
vault.mkdir(parents=True, exist_ok=True)
target = vault / "benchmark-5mb.md"
goal = 5_000_000
with target.open("wb") as output:
    index = 0
    while output.tell() < goal:
        if index % 200 == 0:
            line = f"\n## Section {index // 200}: Architecture and memory consumption\n\n"
        elif index % 113 == 0:
            line = "| Stage | Description | Status |\n|---|---|---|\n| Index | Local search | Running |\n"
        elif index % 47 == 0:
            line = f"- [ ] Review note {index} and cross-reference [[Section {index % 29}]]\n"
        else:
            line = (
                f"Paragraph {index}: A local-first Markdown note with clear sentences and "
                "regular text. The reader keeps writing, searching, and reading. "
                "Русский текст для проверки форматирования, ввода и памяти. "
                "No remote pictures or external dependencies. \n"
            )
        output.write(line.encode("utf-8"))
        index += 1
print(f"AQUILUM_FIXTURE path={target} bytes={target.stat().st_size} paragraphs={index}")
