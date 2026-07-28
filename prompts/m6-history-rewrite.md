# Goal prompt spec: M6-REWRITE — history identity rewrite (G½)

Authored at the top tier (Cowork bridge) 2026-07-28, on the owner's
explicit decisions after G phase 1's clean scan (22b8e96):
- **D7:** rewrite-then-flip. Author/committer `justin.parsons919@gmail.com`
  → **`justin.parsons@cipherpine.com`** across all history.
- Strip every `Claude-Session:` trailer line from commit messages.
  **`Co-Authored-By:` lines stay** — the AI collaboration remains
  honestly attributed.
- **D4-final:** everything publishes; `prompts/` stays in place. Nothing
  else about history changes.

This runs BETWEEN m6-public-flip.md's phase 1 (done) and phase 2.

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): this is the most dangerous operation of the program — a
force-push replacing every SHA on main. It is a metadata rewrite ONLY:
the invariants in R5 prove no tree (file content) changed. Installing
`git-filter-repo` is an owner-sanctioned dev-tool install for this one
operation, not a project dependency — §4.2 is not implicated and
`Cargo.*` must not change. Any invariant failure in R5 = STOP before the
push; the backup makes every state recoverable.

## PRECONDITIONS (mismatch = STOP and report)

P1 `main` tip is 22b8e96, tree clean EXCEPT:
   ```
    M prompts/m6-launchers.md
   ?? prompts/m6-history-rewrite.md
   ```
P2 `git tag` is empty (no tags to rewrite), and the only non-main ref on
   origin is `dependabot/cargo/serde_json-1.0.151`.

## PHASE 0 — commit this spec

`prompts/m6-history-rewrite.md` + `prompts/m6-launchers.md`:
    docs(prompts): add M6-REWRITE spec (G½ — identity rewrite)

## R0 — resolve the dependabot branch FIRST

A ref left behind would keep unrewritten (old-email) history alive on the
remote. `gh pr view` the serde_json 1.0.151 PR: if it is a pure version
bump of an existing dependency AND its CI run is green, merge it per
DECISIONS.md §3 (this is the pre-approved case), then `git pull`. If it
is anything else, close the PR and delete the remote branch, noting
dependabot will re-raise it. Either way: after this step,
`git ls-remote --heads origin` shows main only (or main + nothing).

## R1 — BACKUP (nothing proceeds until this verifies)

```
git clone --mirror . ..\quotapane-pre-rewrite-backup.git
git -C ..\quotapane-pre-rewrite-backup.git fsck --full
git -C ..\quotapane-pre-rewrite-backup.git rev-parse HEAD
```
fsck must be clean; record the backup HEAD. The backup stays PRIVATE on
this machine — it intentionally preserves the old email; never push it.

## R2 — tool

`pip install git-filter-repo` (or the single-file script). Record
`git filter-repo --version` in the end gate.

## R3 — capture BEFORE invariants (to files, not eyeballs)

```
git rev-list --count main                          > inv_count_before
git log --format=%T main                           > inv_trees_before
git log --format=%B main | grep -c "Co-Authored-By" > inv_coauth_before
git log --format=%B main | grep -c "Claude-Session:" > inv_sess_before
git rev-parse main                                 > inv_tip_before
```

## R4 — the rewrite

filter-repo refuses to run on a clone with a remote unless forced and
strips `origin` when done — expected; you re-add it in R6. Run exactly:

```
git filter-repo --force \
  --email-callback "return email.replace(b'justin.parsons919@gmail.com', b'justin.parsons@cipherpine.com')" \
  --message-callback "lines = [l for l in message.split(b'\n') if not l.startswith(b'Claude-Session:')]
while lines and lines[-1] == b'':
    lines.pop()
return b'\n'.join(lines) + b'\n'"
```

The email callback applies to BOTH author and committer. It must not
touch `noreply@github.com` or the dependabot address (replace() cannot —
they don't contain the old string; verify anyway in R5).

## R5 — AFTER invariants (ANY failure = STOP; do not push; report)

- `git log --format='%ae%n%ce' main | sort -u` → EXACTLY
  `justin.parsons@cipherpine.com` plus (if present from merges)
  `noreply@github.com` and the dependabot noreply. The gmail: zero hits.
- `git log --format=%B main | grep -c "Claude-Session:"` → 0.
- Co-Authored-By count == inv_coauth_before.
- `git rev-list --count main` == inv_count_before.
- `git log --format=%T main` is BYTE-IDENTICAL to inv_trees_before —
  every tree unchanged, order preserved: the proof this rewrite touched
  metadata only, no file content anywhere in history.
- `git status` clean; `cargo test --workspace` green (147).

## R6 — SHA map, config, push

- `git config user.email justin.parsons@cipherpine.com` (repo-local),
  and confirm `git config user.name` is what the owner wants public.
- Copy `.git/filter-repo/commit-map` → `prompts/m6-sha-map.txt`, prepend
  a two-line header ("old → new, identity rewrite 2026-07-28; every SHA
  cited in DECISIONS.md and prompts/*.md resolves through this map").
  Commit (this commit already carries the new address):
      docs: SHA map for the 2026-07-28 history identity rewrite
- `git remote add origin https://github.com/cipherpine/quotapane.git`
- `git push --force origin main` — branch protection is not yet enabled
  (the owner's list turns it on AFTER the flip), so this succeeds.
- CI green on the force-pushed tip.

## STANDING RULE FOR ALL LATER SESSIONS (authorized here)

Every SHA pinned in `prompts/*.md` and `DECISIONS.md` predates this
rewrite. Later sessions verify such pins BY TRANSPOSING through
`prompts/m6-sha-map.txt`; a pin that resolves through the map is
satisfied, not a §4.7 conflict. m6-public-flip.md phase 2 proceeds this
way on the owner's go-ahead.

## RESIDUALS — report, don't fix

GitHub server-side may retain pre-rewrite objects (dangling, fetchable
only by exact old SHA) and `refs/pull/*` from dependabot PRs. The repo
was private for its entire pre-rewrite life, so no outsider holds those
SHAs; the owner can ask GitHub support to run a GC after the flip for
belt-and-braces. The local mirror backup also holds them, deliberately.

## DO NOT

Run R4 before R1's fsck passes; push before every R5 invariant holds;
push or publish the backup; touch any file content; add anything to
Cargo.*; enable any GitHub setting; start m6-public-flip phase 2; tag.

## END GATE — STOP

Report: backup path + backup HEAD; filter-repo version; the
before/after invariant table; old tip (22b8e96 + R0 outcome) → new tip;
the sha-map commit; CI run on the new tip; residuals restated. Then STOP
— the owner reviews on GitHub, then gives the phase-2 go-ahead.
