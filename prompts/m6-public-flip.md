# Goal prompt spec: M6-PUBLIC — history scan, hygiene, and the flip

Authored at the top tier (Cowork bridge) 2026-07-28. This is **Prompt G**
under the REORDERED gates (owner decision 2026-07-27: flip before v1.0.0);
it SUPERSEDES the "Prompt G — public flip" section of
`prompts/m6-prompts-b-to-g.md`, whose preconditions still describe the old
order (v1.0.0 already tagged). That section is now historical; this file
governs.

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): Phase 1 is read-only plus one report file. Phase 2 runs
ONLY on the owner's explicit go-ahead in a later turn, after they resolve
D7 on phase 1's evidence; its `.github/**` bytes are §4.1 and are embedded
verbatim in §W below (route b — transcribe, verify md5, commit; author
nothing), while `CODE_OF_CONDUCT.md` is yours to author. The GitHub
settings changes and the visibility flip itself are the OWNER'S HANDS in
the web UI — you never touch them (§4.8).

## PRECONDITIONS (mismatch = STOP and report)

P1 `main` tip is e2ac7d4 (Prompt E's last commit), CI green (7 check
   runs), and `git status --porcelain` shows EXACTLY:

   ```
    M prompts/m6-launchers.md
   ?? prompts/m6-public-flip.md
   ```

P2 Prompts B, D, E all landed (verify by log, not trust): 81bc17b,
   ea134d6, e2ac7d4 are ancestors of main.

## PHASE 0 — commit this spec

`prompts/m6-public-flip.md` + `prompts/m6-launchers.md`:
    docs(prompts): add M6-PUBLIC spec and launcher (Prompt G, reordered)

## PHASE 1 — FULL-HISTORY SECRET SCAN (read-only; then HARD STOP)

Context you should hold: the CI gitleaks job (Prompt D) already scanned
all 36 commits across every ref, green. Phase 1 is the INDEPENDENT second
opinion it cannot give: this project's own credential shapes, deleted
files included, plus the exposure inventory that decides what a public
history reveals.

- Input set: `git rev-list --all` — every commit on every ref, including
  remote-only refs (fetch first; the dependabot branch counts).
- Pass A: run the same pinned gitleaks binary locally (version + SHA256
  from `ci.yml`) over the full history — record version and exit status.
- Pass B: independent grep of every historical blob for the credential
  shapes THIS project handles: `sk-ant-`, `sk-` followed by 20+
  token-safe chars, OAuth refresh-token shapes, `Bearer ` + long token,
  and the exact key NAMES from the credential parse structs (names only —
  §4.4: NEVER print a candidate value; report file + commit + line + key
  name and whether it is a known synthetic fixture).
- Deleted-file coverage: a token committed then removed is still in
  history and is exactly what this pass exists for.
- Exposure inventory: every path that ever existed on any ref and is not
  in the current tree (`prompts/` history, `_claude_setup` if ever
  tracked, anything surprising) — the owner reads this as "what a public
  clone can retrieve."
- Write `prompts/m6-history-scan.md`: both passes' results, every hit
  dispositioned (synthetic fixture vs NOT), the exposure inventory, and
  your argued read on whether this history is safe to publish. "I am not
  certain" is a valid and reportable conclusion.
- Commit: `M6-public: full-history secret scan and exposure inventory`
- Push. CI green. **STOP. Report the scan verdict prominently.** D7
  (rename-in-place vs fresh repo) and D4-final (publish prompts/ or not)
  are the owner's, decided on this report.

## §W — PHASE 2 BYTES (pre-authored; transcribe only on owner go-ahead)

md5 gates:

| file | md5 |
|---|---|
| `.github/ISSUE_TEMPLATE/bug_report.yml` | `eca2e1ad7c55d85e2b45bf680cea1315` |
| `.github/ISSUE_TEMPLATE/feature_request.yml` | `a8c53a94d1b96afa69b3e56392ff0705` |
| `.github/PULL_REQUEST_TEMPLATE.md` | `fecbff22068b9bf9e1ad1590fabf2de6` |

`.github/ISSUE_TEMPLATE/bug_report.yml` (2069 bytes):

````yaml
name: Bug report
description: Something broke, rendered wrong, or behaved unexpectedly
labels: [bug]
body:
  - type: markdown
    attributes:
      value: |
        **Never paste credential material.** No tokens, no `auth.json` or
        `.credentials.json` contents, no `Authorization` headers, no
        environment dumps. QuotaPane's own outputs are designed to contain
        no secrets, but skim anything you paste before you post it. A report
        that includes a credential will be edited/deleted on sight — and you
        should rotate that credential immediately.
  - type: input
    id: version
    attributes:
      label: Version
      description: Output of `quotapane-cli --version` (or the release tag you downloaded).
      placeholder: quotapane-cli 1.0.0
    validations:
      required: true
  - type: dropdown
    id: os
    attributes:
      label: Operating system
      options:
        - Windows (release binary)
        - Linux (release binary)
        - Built from source (any OS — say which below)
    validations:
      required: true
  - type: dropdown
    id: provider
    attributes:
      label: Area
      options:
        - Claude provider
        - Codex provider
        - Both providers
        - Window / rendering
        - CLI
        - Other
    validations:
      required: true
  - type: textarea
    id: what
    attributes:
      label: What happened, and what did you expect?
    validations:
      required: true
  - type: textarea
    id: output
    attributes:
      label: Normalized CLI output (optional)
      description: >
        If relevant, paste the output of `quotapane-cli --once --json`.
        This is the normalized snapshot — percentages and reset times only,
        no token material by design — but read it before posting anyway.
      render: json
  - type: checkboxes
    id: no-secrets
    attributes:
      label: Credential check
      options:
        - label: I confirm this report contains no tokens, credential-file contents, or other secret material.
          required: true
````

`.github/ISSUE_TEMPLATE/feature_request.yml` (803 bytes):

````yaml
name: Feature request
description: Suggest an improvement or new capability
labels: [enhancement]
body:
  - type: textarea
    id: problem
    attributes:
      label: What problem would this solve for you?
    validations:
      required: true
  - type: textarea
    id: proposal
    attributes:
      label: What would you like to see?
    validations:
      required: true
  - type: markdown
    attributes:
      value: |
        **Scope note:** this project's entire premise is a small, auditable
        trust boundary (`SECURITY.md`). A feature that would read new
        credential sources, contact new hosts, add a dependency with network
        or serialization capability, or persist new data gets a threat-model
        pass before any code — expect that conversation, not a fast merge.
````

`.github/PULL_REQUEST_TEMPLATE.md` (1017 bytes):

````markdown
## What & why

<!-- What does this change, and what problem does it solve? -->

## Checklist

- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked` all pass locally.
- [ ] This PR does **not** touch a protected path (`crates/usage-core/src/egress/**`, `crates/usage-core/src/credentials/**`, any security-invariant test, `deny.toml`, `SECURITY.md`, `THREAT_MODEL.md`, `.github/**`, `.cargo/**`) — or, if it must, I have said so explicitly below and expect maintainer review of every byte.
- [ ] Any `Cargo.lock` change is a justified dependency change with a matching row in `CONTRIBUTING.md`'s table, not incidental churn.

## Security invariant statement (required)

<!-- Name the SECURITY.md invariant (1–7) your change could most plausibly
     affect, and say in one or two sentences why it does not weaken it.
     "None plausibly affected" is a valid answer for pure-UI/docs changes —
     but say it, don't skip it. -->
````

## PHASE 2 — hygiene (ONLY on the owner's explicit go-ahead, after D7)

1. §4a — transcribe the three §W files byte-exactly, verify the md5
   table, commit:
       docs(github): issue and PR templates — credential-safe by design
   Design intent you are verifying, not editing: the bug template REFUSES
   credential material explicitly and asks only for version/OS/area and
   the normalized CLI output; the PR template demands a security-invariant
   statement and a protected-paths declaration.
2. YOURS — `CODE_OF_CONDUCT.md`: Contributor Covenant 2.1, enforcement
   contact = "report privately to the maintainer via GitHub". Commit:
       docs: add CODE_OF_CONDUCT (Contributor Covenant 2.1)
3. If D7 resolved to rename-in-place AND D4-final to publish prompts/:
   nothing moves; note it. If the owner instead ordered moves, execute
   exactly what they specified — no improvisation; a move they did not
   name is a §4.7 stop. The empty `_to_delete/` dir at the repo root is
   untracked; tell the owner to delete it locally.
4. Push. CI green.

## OWNER'S LIST (report verbatim at the end gate; do not do any of it)

In the GitHub web UI, in this order: enable private vulnerability
reporting; enable branch protection on `main` (require the CI checks,
restrict force-push); confirm Actions workflow permissions allow
`id-token: write` + `attestations: write` for release.yml; **flip
visibility to public**; then confirm the repo's Releases page is empty
(v1.0.0 comes next via Prompt F). Also owner-local: register `quotapane`
on crates.io; delete `_to_delete/`.

## DO NOT

Flip visibility or change any GitHub setting; move or delete any file the
owner did not name; print any candidate secret value (§4.4 — names and
locations only); add a gitleaks config; touch code; tag anything; start
Prompt F.

## END GATE — STOP

Phase 1: the verdict (hits, dispositions, exposure inventory, your argued
safe-to-publish read). Phase 2 (if reached): the commit SHAs, CI, and the
owner's list above, restated. Then STOP — the flip is the owner's hands,
and Prompt F only makes sense on a public repo.
