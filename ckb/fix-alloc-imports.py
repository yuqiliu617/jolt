#!/usr/bin/env python3
"""Inserts `use alloc::...` imports driven by rustc cannot-find errors.

Usage: python ckb/fix-alloc-imports.py <cargo check args...>
Runs cargo check repeatedly, parsing E0405/E0412/E0425/E0433/E0599 errors for
known alloc-prelude items, and inserts tailored imports at the top of each
offending file. Stops when no fixable errors remain.
"""

import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

SYMBOL_PATHS = {
    "Vec": "alloc::vec::Vec",
    "String": "alloc::string::String",
    "Box": "alloc::boxed::Box",
    "ToString": "alloc::string::ToString",
    "ToOwned": "alloc::borrow::ToOwned",
    "Cow": "alloc::borrow::Cow",
    "Arc": "alloc::sync::Arc",
    "Rc": "alloc::rc::Rc",
    "BTreeMap": "alloc::collections::BTreeMap",
    "BTreeSet": "alloc::collections::BTreeSet",
    "VecDeque": "alloc::collections::VecDeque",
}

METHOD_TRAITS = {
    "to_string": "alloc::string::ToString",
    "to_owned": "alloc::borrow::ToOwned",
}

ERR_RE = re.compile(
    r"error\[E0(?:405|412|425|433)\]: cannot find (?:type|value|trait|struct|crate or module) `(\w+)`"
)
METHOD_RE = re.compile(r"error\[E0599\].*no method named `(\w+)`")
LOC_RE = re.compile(r"^\s*-->\s+(.+?):(\d+):\d+")


def run_check(args):
    proc = subprocess.run(
        ["cargo", "check", *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    return proc.stderr


def collect_fixes(stderr):
    fixes = defaultdict(set)
    pending = None
    for line in stderr.splitlines():
        m = ERR_RE.search(line)
        if m and m.group(1) in SYMBOL_PATHS:
            pending = SYMBOL_PATHS[m.group(1)]
            continue
        m = METHOD_RE.search(line)
        if m and m.group(1) in METHOD_TRAITS:
            pending = METHOD_TRAITS[m.group(1)]
            continue
        m = LOC_RE.match(line)
        if m and pending:
            fixes[m.group(1).replace("\\", "/")].add(pending)
            pending = None
    return fixes


def insert_imports(path, imports):
    p = Path(path)
    text = p.read_text(encoding="utf-8")
    lines = text.splitlines(keepends=True)
    existing = {imp for imp in imports if f"use {imp}" in text}
    needed = sorted(imports - existing)
    if not needed:
        return False
    # Insert before the first `use ` line (and before any attributes attached
    # to it), else after the leading doc/attr block.
    idx = None
    for i, line in enumerate(lines):
        if line.startswith("use "):
            idx = i
            while idx > 0 and lines[idx - 1].lstrip().startswith("#["):
                idx -= 1
            break
    if idx is None:
        idx = 0
        for i, line in enumerate(lines):
            s = line.strip()
            if s and not s.startswith(("//", "#!", "/*", "*")):
                idx = i
                break
    block = "".join(f"use {imp};\n" for imp in needed)
    lines.insert(idx, block)
    p.write_text("".join(lines), encoding="utf-8")
    return True


def main():
    args = sys.argv[1:]
    for round_no in range(12):
        stderr = run_check(args)
        fixes = collect_fixes(stderr)
        changed = False
        for path, imports in fixes.items():
            if insert_imports(path, imports):
                print(f"round {round_no}: {path} += {sorted(imports)}")
                changed = True
        if not changed:
            errors = stderr.count("error[")
            print(f"done after round {round_no}; remaining errors: {errors}")
            if errors:
                tail = [l for l in stderr.splitlines() if "error" in l][:15]
                print("\n".join(tail))
            return 0 if errors == 0 else 1
    print("did not converge")
    return 1


if __name__ == "__main__":
    sys.exit(main())
