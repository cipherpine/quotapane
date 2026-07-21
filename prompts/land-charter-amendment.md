# Goal prompt: LAND CHARTER AMENDMENT + clean CRLF churn

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — its §4 stop conditions override
everything below.

TIER / PROTECTED-PATH NOTE (per DECISIONS.md §6): This session is Sonnet.
It DOES touch §4.1 protected paths, but ONLY as a §4a *verify-and-commit* of
already-reviewed bytes — never authoring. Specifically: (a) it commits the
top-tier-authored DECISIONS.md, and (b) it restores 7 files to their
committed (reviewed) bytes via `git checkout --`. If any step would require
*editing* a protected file rather than committing/restoring it, STOP (§4.1).

PRECONDITIONS (any mismatch = STOP and report):
P1 `git rev-parse --abbrev-ref HEAD` = `main`, tip = 97f0e90.
P2 DECISIONS.md already contains the amendment on disk — verify:
   `grep -n "^## 4a" DECISIONS.md` returns a line, and
   `grep -c "Every goal prompt states the session's model tier" DECISIONS.md`
   returns 1. If not, STOP (the top-tier write didn't land).
P3 `git status --short` shows ONLY these, nothing else:
   - ` M DECISIONS.md`  (the amendment)
   - `?? prompts/land-charter-amendment.md`  (this prompt file)
   - modifications to the 7 churn files listed in PHASE A
   Anything else present = STOP.

PHASE A — clear the stale lock and the CRLF churn:
1. Remove the stale lock left by the cloud bridge (it cannot delete files):
   PowerShell: `Remove-Item .git\index.lock -Force -ErrorAction SilentlyContinue`
   It is a zero-byte stale lock; if `git status` already runs clean, skip.
2. Restore these 7 files to their committed bytes (editor CRLF re-save churn,
   content-identical to HEAD — confirm with
   `git diff --ignore-all-space --stat` showing EMPTY before you do this):
     git checkout -- .github/workflows/ci.yml CONTRIBUTING.md Cargo.toml `
       crates/usage-core/Cargo.toml crates/usage-core/src/model/mod.rs `
       crates/usage-core/src/providers/mod.rs deny.toml
   If `git diff --ignore-all-space --stat` is NOT empty (i.e. a real content
   change hides in the churn), STOP and report — do not discard it.

PHASE B — commit the charter + this prompt (NOT the churn):
3. `git status --short` must now show ONLY:
   ` M DECISIONS.md` and `?? prompts/land-charter-amendment.md`.
   The 7 files must be gone from the list. If anything else remains, STOP.
4. `git diff DECISIONS.md` — confirm the additions are limited to: the new
   `## 4a` section, the new §6 tier-declaration paragraph, and the §2 roadmap
   line marking M3 accepted + the queued look item. No other edits.
5. Stage ONLY those two paths and commit:
     git add DECISIONS.md prompts/land-charter-amendment.md
     git commit -m "docs: charter §4a (verify, don't author) + §6 tier rule; mark M3 accepted"
6. `git push`. Record the commit SHA and the Actions run URL. CI is docs-only
   here (nothing added is in the build) — all jobs should stay green;
   unexplained red = DECISIONS.md §4.6, STOP.

END GATE — STOP HERE. Report: the commit SHA, the CI run URL + job results,
confirmation that `git status` is clean, and any deviation. Do NOT start M3.5
tray work. Never touch §4.1 paths beyond the restore/commit above. Never
capture the screen.
