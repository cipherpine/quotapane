#!/usr/bin/env python3
"""M13 P1 §4a patch applier — extracts the two top-tier-authored patches from
prompts/m13-pace-followons.md's own bytes and applies them verbatim.

Nothing here is retyped: OLD/NEW come from the spec file, and the script
refuses to write unless OLD occurs EXACTLY ONCE in the target and NEW occurs
exactly once afterwards. Run with --verify to check only (no writes).

Scratch tool for one commit; not part of CI.
"""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC = ROOT / "prompts" / "m13-pace-followons.md"

# (target, index of the OLD/NEW pair in the spec, in document order)
TARGETS = [
    (ROOT / "SECURITY.md", 0),
    (ROOT / "invariants.manifest", 1),
]

verify_only = "--verify" in sys.argv

spec = SPEC.read_text(encoding="utf-8")
pairs = []
old = None
for line in spec.split("\n"):
    if line.startswith("OLD: "):
        old = line[len("OLD: "):]
    elif line.startswith("NEW: "):
        if old is None:
            sys.exit("spec: NEW: without a preceding OLD:")
        pairs.append((old, line[len("NEW: "):]))
        old = None

if len(pairs) != len(TARGETS):
    sys.exit(f"spec: expected {len(TARGETS)} OLD/NEW pairs, found {len(pairs)}")

failures = []
for target, index in TARGETS:
    old, new = pairs[index]
    text = target.read_text(encoding="utf-8")
    n_old, n_new = text.count(old), text.count(new)
    rel = target.relative_to(ROOT).as_posix()
    if n_new == 1 and n_old == 0:
        print(f"OK   {rel}: already patched (NEW unique, OLD absent)")
        continue
    if n_old != 1:
        failures.append(f"{rel}: OLD occurs {n_old} times (must be exactly 1)")
        continue
    if n_new != 0:
        failures.append(f"{rel}: NEW already occurs {n_new} times before patching")
        continue
    if verify_only:
        print(f"PEND {rel}: OLD unique, NEW absent — would patch")
        continue
    patched = text.replace(old, new)
    if patched.count(new) != 1:
        failures.append(f"{rel}: NEW is not unique after patching")
        continue
    target.write_bytes(patched.encode("utf-8"))
    print(f"OK   {rel}: patched (OLD was unique, NEW is unique)")

if failures:
    for f in failures:
        print(f"FAIL {f}", file=sys.stderr)
    sys.exit(1)
print("all patches accounted for")
