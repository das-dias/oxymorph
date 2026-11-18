#!/usr/bin/env python3
"""Extract a parity corpus from the vendored morphnet.

Run with `uv run python tools/extract_oracle.py` from `vendor/morphnet` (so the
upstream package is importable).  Harvests netlist files and all inline netlist
strings found in the upstream test suite, runs the upstream Python as the oracle,
and writes `tests/corpus/cases.json` with the expected IR (JSON) and writer text.

Core tests then assert against the committed fixtures with no Python toolchain.
"""
from __future__ import annotations

import ast
import json
import sys
from pathlib import Path

UPSTREAM = Path(__file__).resolve().parent.parent / "vendor" / "morphnet"
OUT = Path(__file__).resolve().parent.parent / "tests" / "corpus" / "cases.json"

PARSERS = {
    "spice": ("morphnet.netlist.spice.parser", "parse_spice"),
    "hspice": ("morphnet.netlist.hspice.parser", "parse_hspice"),
    "xyce": ("morphnet.netlist.xyce.parser", "parse_xyce"),
    "spectre": ("morphnet.netlist.spectre.parser", "parse_spectre"),
    "spectre_spice": ("morphnet.netlist.spectre_spice.parser", "parse_spectre_spice"),
    "vams": ("morphnet.netlist.vams.parser", "parse_vams"),
}
WRITERS = {
    "spice": ("morphnet.netlist.spice.writer", "write_spice"),
    "hspice": ("morphnet.netlist.hspice.writer", "write_hspice"),
    "xyce": ("morphnet.netlist.xyce.writer", "write_xyce"),
    "spectre": ("morphnet.netlist.spectre.writer", "write_spectre"),
    "vams": ("morphnet.netlist.vams.writer", "write_vams"),
    "spectre_spice": ("morphnet.netlist.spectre_spice.writer", "write_spectre_spice"),
}


def load(modpath: str, name: str):
    import importlib

    return getattr(importlib.import_module(modpath), name)


def harvest_inline_strings() -> list[str]:
    """All string constants in the upstream test suite that look like netlists."""
    strings: set[str] = set()
    for test in sorted((UPSTREAM / "tests").glob("test_*.py")):
        try:
            tree = ast.parse(test.read_text())
        except SyntaxError:
            continue
        for node in ast.walk(tree):
            if isinstance(node, ast.Constant) and isinstance(node.value, str):
                s = node.value
                if "\n" in s and 0 < len(s) < 8000:
                    strings.add(s)
    return sorted(strings)


def main() -> int:
    sys.path.insert(0, str(UPSTREAM / "src"))
    parsers = {k: load(*v) for k, v in PARSERS.items()}
    writers = {k: load(*v) for k, v in WRITERS.items()}

    cases: list[dict] = []

    def add(fmt: str, name: str, text: str) -> None:
        try:
            circuit = parsers[fmt](text)
        except Exception:
            return
        try:
            expected_json = json.loads(circuit.to_json())
        except Exception:
            return
        try:
            proto = circuit.to_proto_bytes().hex()
        except Exception:
            proto = None
        writes: dict[str, str] = {}
        for wfmt, wfn in writers.items():
            try:
                writes[wfmt] = wfn(circuit)
            except Exception:
                pass
        cases.append(
            {
                "format": fmt,
                "name": name,
                "input": text,
                "json": expected_json,
                "proto": proto,
                "writes": writes,
            }
        )

    # 1. Committed netlist files (provenance is the path).
    netlist_glob = {
        "spectre": "*.scs",
        "hspice": "*.sp",
        "vams": "*.vams",
    }
    for fmt, pat in netlist_glob.items():
        for f in sorted((UPSTREAM / "tests" / "netlists" / fmt).glob(pat)):
            add(fmt, f"netlist/{fmt}/{f.name}", f.read_text())

    # 2. Inline netlists from the test suite, probed against every parser.
    for i, s in enumerate(harvest_inline_strings()):
        for fmt in parsers:
            before = len(cases)
            add(fmt, f"inline/{i}", s)

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(cases, indent=1) + "\n")
    by_fmt: dict[str, int] = {}
    for c in cases:
        by_fmt[c["format"]] = by_fmt.get(c["format"], 0) + 1
    print(f"wrote {len(cases)} cases to {OUT}")
    for k in sorted(by_fmt):
        print(f"  {k}: {by_fmt[k]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
