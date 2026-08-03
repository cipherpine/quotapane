#!/usr/bin/env python3
"""Invariant traceability checker (CI job: invariants).

Enforces that invariants.manifest, SECURITY.md's numbered invariant list,
and the `// INV:<ids>` tags on tests in the tree all agree; that every
manifest-named test is LIVE (uncommented, carries an uncommented #[test],
and is not #[ignore]d); and that each invariant's SECURITY.md claim
headline (the bold lead phrase) matches the manifest summary. Stdlib only.
Exit 0 = coherent; exit 1 = drift, with every violation listed.

Guarantee limits, stated so they are not overread (M11-VERIFY F1/F2):
prose BEYOND the claim headline is not machine-checked, and a test's
BODY is not judged — human review of SECURITY.md prose and of test
content remains load-bearing.

This file is a §4.1 protected path — top tier only.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
errors = []


def err(msg: str) -> None:
    errors.append(msg)


# ---------- 1. Parse invariants.manifest ----------
manifest_path = ROOT / "invariants.manifest"
if not manifest_path.exists():
    print("FAIL: invariants.manifest not found", file=sys.stderr)
    sys.exit(1)

inv_ids = []            # [int]
inv_summary = {}        # id -> one-line claim summary
inv_kind = {}           # id -> "test-backed" | "absence"
inv_enforced = {}       # id -> str
manifest_pairs = set()  # (id, "path::fn")
current = None

for lineno, raw in enumerate(manifest_path.read_text(encoding="utf-8").splitlines(), 1):
    line = raw.rstrip()
    if not line or line.startswith("#"):
        continue
    m = re.match(r"^invariant (\d+): (.+)$", line)
    if m:
        current = int(m.group(1))
        if current in inv_ids:
            err(f"manifest:{lineno}: duplicate invariant id {current}")
        inv_ids.append(current)
        inv_summary[current] = m.group(2).strip()
        continue
    if current is None:
        err(f"manifest:{lineno}: content before any invariant block: {line!r}")
        continue
    if line.startswith("kind: "):
        kind = line[len("kind: "):].strip()
        if kind not in ("test-backed", "absence"):
            err(f"manifest:{lineno}: invariant {current}: unknown kind {kind!r}")
        inv_kind[current] = kind
    elif line.startswith("enforced-by: "):
        inv_enforced[current] = line[len("enforced-by: "):].strip()
    elif line.startswith("test: "):
        entry = line[len("test: "):].strip()
        if "::" not in entry:
            err(f"manifest:{lineno}: invariant {current}: malformed test entry {entry!r}")
        else:
            manifest_pairs.add((current, entry))
    else:
        err(f"manifest:{lineno}: unrecognized line: {line!r}")

# ---------- 2. Structural rules ----------
for i in inv_ids:
    if i not in inv_kind:
        err(f"invariant {i}: missing kind")
    if inv_kind.get(i) == "absence" and i not in inv_enforced:
        err(f"invariant {i}: kind=absence requires enforced-by")
    if inv_kind.get(i) == "test-backed" and not any(p[0] == i for p in manifest_pairs):
        err(f"invariant {i}: kind=test-backed but lists no tests")

# ---------- 3. SECURITY.md numbered list must match the id set ----------
sec = (ROOT / "SECURITY.md").read_text(encoding="utf-8")
m = re.search(r"^## Security invariants\n(.*?)(?=^## )", sec, re.S | re.M)
if not m:
    err("SECURITY.md: could not locate the '## Security invariants' section")
else:
    sec_ids = [int(n) for n in re.findall(r"^(\d+)\. \*\*", m.group(1), re.M)]
    # Claim headlines: "N. **Bold lead phrase.**" -> head = text up to the
    # first "." or " — ", which must prefix the manifest summary (F2).
    sec_heads = {int(n): t for n, t in
                 re.findall(r"^(\d+)\. \*\*(.+?)\*\*", m.group(1), re.M)}
    if sec_ids != sorted(set(sec_ids)):
        err(f"SECURITY.md: invariant numbering is not strictly increasing: {sec_ids}")
    if set(sec_ids) != set(inv_ids):
        err(f"id drift: SECURITY.md lists invariants {sorted(set(sec_ids))} "
            f"but the manifest lists {sorted(set(inv_ids))}")
    for i, head in sorted(sec_heads.items()):
        stop = len(head)
        for sep in (".", " \u2014 "):
            k = head.find(sep)
            if k != -1:
                stop = min(stop, k)
        head = head[:stop].strip()
        summary = inv_summary.get(i, "")
        if head and not summary.startswith(head):
            err(f"invariant {i}: claim-headline drift — SECURITY.md says "
                f"{head!r} but the manifest summary starts "
                f"{summary[:60]!r}")

# ---------- 4. Every manifest test exists in the tree ----------
def is_comment(line: str) -> bool:
    return line.lstrip().startswith("//")


def find_live_test(lines, fn):
    """Return (found, problems) for fn as a LIVE test in these lines."""
    pat = re.compile(rf"\bfn {re.escape(fn)}\s*\(")
    for i, line in enumerate(lines):
        if not pat.search(line) or is_comment(line):
            continue
        # Walk the attribute/comment block above the fn.
        has_test, has_ignore = False, False
        j = i - 1
        while j >= 0:
            stripped = lines[j].strip()
            if stripped.startswith("#["):
                if "#[test]" in stripped:
                    has_test = True
                if "ignore" in stripped:
                    has_ignore = True
                j -= 1
            elif is_comment(lines[j]) or not stripped:
                j -= 1
            else:
                break
        problems = []
        if not has_test:
            problems.append("no uncommented #[test] attribute above it")
        if has_ignore:
            problems.append("carries #[ignore] — the test does not run")
        return True, problems
    return False, []


lines_cache = {}
for inv, entry in sorted(manifest_pairs):
    path_s, fn = entry.rsplit("::", 1)
    p = ROOT / path_s
    if not p.exists():
        err(f"invariant {inv}: test file missing: {path_s}")
        continue
    lines = lines_cache.setdefault(
        path_s, p.read_text(encoding="utf-8").splitlines())
    found, problems = find_live_test(lines, fn)
    if not found:
        err(f"invariant {inv}: fn {fn} not found LIVE (uncommented) in {path_s}")
    for prob in problems:
        err(f"invariant {inv}: fn {fn} in {path_s}: {prob}")

# ---------- 5. Source tags <-> manifest, set-equal both directions ----------
tag_pairs = set()
for p in sorted((ROOT / "crates").rglob("*.rs")):
    rel = p.relative_to(ROOT).as_posix()
    lines = p.read_text(encoding="utf-8").splitlines()
    for i, line in enumerate(lines):
        tm = re.search(r"//\s*INV:([0-9,]+)", line)
        if not tm:
            continue
        ids = [int(x) for x in tm.group(1).split(",") if x]
        if line.lstrip().startswith("//") and "INV:" in line and \
                not line.lstrip().startswith("// INV:"):
            # A tag inside ordinary commented-out code (e.g. "// // INV:2")
            # is dead text; skip it so dead blocks cannot satisfy set-equality.
            continue
        fn_name = None
        for j in range(i + 1, min(i + 6, len(lines))):
            cand = lines[j]
            if cand.lstrip().startswith("//"):
                continue
            fm = re.search(r"\bfn (\w+)\s*\(", cand)
            if fm:
                fn_name = fm.group(1)
                break
        if fn_name is None:
            err(f"{rel}:{i+1}: // INV tag with no fn within 5 lines")
            continue
        for inv in ids:
            tag_pairs.add((inv, f"{rel}::{fn_name}"))

for pair in sorted(manifest_pairs - tag_pairs):
    err(f"manifest lists {pair[1]} for invariant {pair[0]} "
        f"but the source carries no matching // INV:{pair[0]} tag")
for pair in sorted(tag_pairs - manifest_pairs):
    err(f"source tags {pair[1]} with // INV:{pair[0]} "
        f"but the manifest has no such entry")

# ---------- 6. ci-job references must exist ----------
ci = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
for inv, text in sorted(inv_enforced.items()):
    jm = re.search(r"ci-job ([\w-]+)", text)
    if jm and jm.group(1) not in ci:
        err(f"invariant {inv}: enforced-by names ci-job {jm.group(1)!r}, "
            f"absent from .github/workflows/ci.yml")

# ---------- verdict ----------
if errors:
    print(f"FAIL: invariant traceability broken ({len(errors)} violation(s)):",
          file=sys.stderr)
    for e in errors:
        print(f"  - {e}", file=sys.stderr)
    sys.exit(1)

print(f"OK: {len(inv_ids)} invariants, {len(manifest_pairs)} test bindings, "
      f"tags and manifest set-equal, SECURITY.md id set matches.")
