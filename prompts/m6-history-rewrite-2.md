# Goal prompt spec: M6-REWRITE-2 — author display name (G½b)

Authored at the top tier (Cowork bridge) 2026-07-28, on the owner's
decision after G½: author/committer display name
**`justinparsons919` → `Justin Parsons`** across all history. Email stays
`justin.parsons@cipherpine.com` (already rewritten in G½). Message bodies
are untouched this time. Same machinery, same discipline, one variable.

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): identical risk class to G½ — a metadata-only rewrite plus
force-push, proven by the same tree-hash invariant. SHA pins from before
G½ transpose through `prompts/m6-sha-map.txt`; after THIS rewrite they
transpose through that map AND the new `prompts/m6-sha-map-2.txt`, chained
in order — the standing rule from G½ extends to the chain, authorized
here.

## PRECONDITIONS (mismatch = STOP and report)

P1 `main` tip is 481b270 (G½'s sha-map commit), CI green
   (run 30389136652), tree clean EXCEPT:
   ```
    M prompts/m6-launchers.md
   ?? prompts/m6-history-rewrite-2.md
   ```
P2 `git log --format='%an' main | sort -u` contains `justinparsons919`
   and `dependabot[bot]` and nothing else. If any third name appears,
   STOP and report it — the callback below maps ONE exact string.
P3 The G½ backup (`..\quotapane-pre-rewrite-backup.git`) still exists.
   Do not touch it.

## PHASE 0 — commit this spec

`prompts/m6-history-rewrite-2.md` + `prompts/m6-launchers.md`:
    docs(prompts): add M6-REWRITE-2 spec (G½b — author display name)

## R1 — second backup, then invariants BEFORE

```
git clone --mirror . ..\quotapane-pre-rename-backup.git
git -C ..\quotapane-pre-rename-backup.git fsck --full
```
Then capture, exactly as G½ did: rev-list count, ordered `%T` list,
Co-Authored-By count, `%an`/`%cn` name census, tip SHA — to files.

## R2 — the rewrite

```
git filter-repo --force \
  --name-callback "return name.replace(b'justinparsons919', b'Justin Parsons')"
```
Applies to both author and committer names. `dependabot[bot]` and any
GitHub-noreply names do not contain the string and must come through
unchanged (verify in R3, don't assume).

## R3 — invariants AFTER (ANY failure = STOP; do not push)

- `git log --format='%an <%ae>' main | sort -u` → EXACTLY
  `Justin Parsons <justin.parsons@cipherpine.com>` and
  `dependabot[bot] <49699333+dependabot[bot]@users.noreply.github.com>`
  (plus a GitHub-noreply committer identity if one exists). Zero hits for
  `justinparsons919` anywhere in `%an`/`%cn` on any ref.
- Commit count unchanged; ordered `%T` tree list BYTE-IDENTICAL;
  Co-Authored-By count unchanged; message bodies untouched
  (`git log --format=%B main` hash-equal before/after).
- `git status` clean; `cargo test --workspace` green (147).

## R4 — SHA map 2, config, push

- `git config user.name "Justin Parsons"` (repo-local), so future
  commits match the rewritten record.
- Copy `.git/filter-repo/commit-map` → `prompts/m6-sha-map-2.txt` with a
  two-line header ("old → new, name rewrite 2026-07-28; chain AFTER
  m6-sha-map.txt when transposing pre-G½ pins"). Commit:
      docs: SHA map 2 for the author-name rewrite
- `git remote add origin https://github.com/cipherpine/quotapane.git`
- `git push --force origin main`. CI green on the new tip.

## DO NOT

Push before every R3 invariant holds; push or publish either backup;
touch message bodies or any file content in history; change the email
(already correct); touch Cargo.*; start m6-public-flip phase 2; tag.

## END GATE — STOP

Report: second backup path; the before/after invariant table; old tip
481b270 → new tip; the sha-map-2 commit; CI run; and confirmation that
`git config user.name`/`user.email` (repo-local) now both match the
rewritten identity. Then STOP — next is the owner's go-ahead for
m6-public-flip phase 2, whose pins transpose through BOTH maps in order.
