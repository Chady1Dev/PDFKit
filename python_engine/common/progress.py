"""
PDFKit Common Progress Reporting and Text Sanitization
"""

import json
import sys
from typing import Optional


def clean_xml_text(s: Optional[str]) -> str:
    """
    Strips illegal XML 1.0 control characters and invalid Unicode codepoints
    (e.g. NULL bytes, 0x00-0x1F except tab/newline/carriage-return, surrogates,
    and non-characters like 0xFFFE, 0xFFFF) to ensure 100% compliance with lxml / Office XML.
    """
    if not s:
        return ""
    # Normalize non-breaking spaces (\xa0) to standard spaces to ensure proper word wrapping
    s = s.replace("\xa0", " ")
    return "".join(
        c
        for c in s
        if ord(c) in (9, 10, 13)
        or (0x20 <= ord(c) <= 0xD7FF)
        or (0xE000 <= ord(c) <= 0xFFFD)
        or (0x10000 <= ord(c) <= 0x10FFFF)
    )


def emit_progress(current: int, total: int, percent: int, stage: str = "converting") -> None:
    """Emits a single-line JSON progress message to stdout."""
    payload = {
        "type": "progress",
        "current": current,
        "total": total,
        "percent": max(0, min(100, percent)),
        "stage": stage,
    }
    print(json.dumps(payload), flush=True)


def emit_success(output_path: str) -> None:
    """Emits a success JSON message to stdout."""
    print(json.dumps({"type": "success", "output": output_path}), flush=True)
