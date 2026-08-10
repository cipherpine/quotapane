# Goal prompt spec: M18a — reach (statusline, gating recipes, WinGet)

Authored at the standing top tier 2026-08-09, from the owner's M18 decision
(2026-08-08): the reach milestone, split so this slice touches **no §4.1
path and no invariant**. The update check and the remaining packaging
targets (Homebrew/AUR) are M18b and are NOT in scope here.

Model tier: top (attended CLI session; the tier permits authoring, and
§1's parser is new code). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

## PRECONDITIONS (mismatch = STOP)

P1 Tip subject "docs: screenshots from this decade" (`0323e1c`), clean
   tree, HEAD == origin/main. Version `1.7.0`. 473 tests
   (cli 64 + cli-integration 13 + core 156 + ui 240).
P2 No `--statusline` flag exists anywhere in the workspace. No
   `packaging/` directory. No `docs/gating.md`.

## Scope fence — read twice

- **No new dependency.** All of §1 is serde_json + std, both already in
  the tree.
- **No §4.1 path.** This slice must not touch `SECURITY.md`,
  `THREAT_MODEL.md`, `invariants.manifest`, `.github/**`, egress,
  credentials, or any security-invariant test. If you find yourself
  needing to, STOP and report — the slice was mis-scoped and the top
  tier needs to know.
- **No version bump, no CHANGELOG entry, no release.** Release timing is
  the owner's; this lands on main.
- **No egress from any new code path.** §1 is a pure stdin formatter.

## §1 — `quotapane-cli --statusline`

A third mode beside `--once` and `--watch`: read one JSON document from
**stdin**, print one line, exit 0. Never polls, never reads a credential
file, never constructs an egress client. This is the zero-egress path:
Claude Code already hands its statusline command the quota numbers.

### Background you must verify before coding

Claude Code's statusLine feature pipes a JSON payload to the configured
command on stdin and displays the first line of stdout. The payload
carries a `rate_limits` field with session/weekly utilization and reset
times. Pin the exact shape from the official Claude Code statusline
documentation (docs.claude.com / code docs) plus these two writeups,
and record in the report what you found:
- nyosegawa.com/en/posts/claude-code-statusline-rate-limits/
- mareksuppa.com/til/claude-code-rate-limits-ccstatusline/
Known caveat to note in docs: `rate_limits` is missing for some
plan/auth combos (anthropics/claude-code#40094).

Whatever the exact keys are, the parser is **defensive by rule**:

- stdin not valid JSON, or `rate_limits` absent/empty → print nothing
  (a single empty line is acceptable if the host requires output) and
  **exit 0**. A statusline must never break its host.
- Unknown extra fields ignored everywhere. Parse only what you print.
- Read percentages and reset times ONLY. The payload also carries cwd,
  model, session id, transcript path — none of it is read, and a test
  proves a sentinel value planted in those fields cannot reach stdout
  (the same welded-aperture idea as invariant 8, enforced here by test
  rather than by a new invariant).

### Output

One line, compact, in the product's voice:

    5h 12% · 7d 48%

- One segment per window present in the payload, session first.
- A segment at or past 80% gains a bang: `7d 83%!`
- If the payload carries a reset time for the *most-used* window, append
  it: `5h 12% · 7d 83%! · resets 2h10m`. Reuse the existing
  countdown-formatting helper if it is reusable from the CLI crate;
  otherwise a small local one — do not move code out of usage-core for
  this.
- Plain text, no ANSI colour in v1 (Claude Code renders it fine either
  way; colour is a follow-up decision, not yours).

### Flags

- `--statusline` conflicts with `--once`, `--watch`, `--json`,
  `--provider`, `--fail-at`, `--debug-raw`, `--allow-proxy` — usage
  error (exit 2) naming the first conflict, same style as the existing
  "exactly one mode" error.
- Update the CLI's `--help`, README's Usage table, and
  `docs/cli-json.md` with one line stating the statusline output is a
  human-format surface NOT covered by the JSON stability contract.

### Tests (usage-cli)

Fixture payloads from the pinned schema, plus: the sentinel test above;
empty/garbage stdin exits 0; the bang threshold at 79/80/81; the
conflict errors; and a test pinning that the statusline code path names
no egress construction (grep-style structural test, same pattern as the
existing "single call site" tests).

## §2 — `docs/gating.md`: quota gates as a practice

`--fail-at` exists and nothing else in the field gates. Write the
recipe page that makes it discoverable. Recipes, each with a bash and a
PowerShell variant where they differ:

1. Pre-flight before a long agent run (`quotapane-cli --once
   --provider all --fail-at 85 || exit 1`) — the one-liner, explained.
2. A CI job step that refuses to start an expensive stage under quota
   pressure (plain YAML fragment, generic — not wired into this repo's
   own CI, which is §4.1).
3. A cron/scheduled-task heartbeat with `--watch` + NDJSON into a log.
4. A git pre-push hook that warns (not blocks) at a threshold.
5. The statusline setup from §1: the settings.json snippet wiring
   `quotapane-cli --statusline` into Claude Code, with the #40094
   caveat stated.

Tone: the README's. Exit codes table linked, not duplicated. Add one
"Gating" line to README's Usage section pointing at the page.

## §3 — WinGet manifests

Create `packaging/winget/` containing the three-file manifest set for
`CipherPine.QuotaPane` version 1.7.0: version, installer, and
defaultLocale manifests. Installer type: zip with portable nested
binaries (both `quotapane.exe` and `quotapane-cli.exe` as portable
commands). InstallerUrl = the real v1.7.0 release asset URL;
InstallerSha256 = the value from the release's own `SHA256SUMS`
(download it, verify against the asset, then copy the hash — never
compute-and-trust without the cross-check).

Validate locally: `winget validate --manifest packaging/winget/...`.
If winget is unavailable in this environment, say so in the report and
leave validation to the owner — do not install anything to get it.

Add `packaging/winget/README.md`: two short sections — how these were
validated, and the owner's submission path (fork microsoft/winget-pkgs,
or `wingetcreate submit`; first submission requires the owner's GitHub
account and the repo's CLA-style agreement, so **the upstream PR is the
owner's act, not this session's**).

## Commits

1. §1 in one commit — code, tests, `--help`, README Usage row,
   cli-json.md note (same-change rule: the mode and every doc line that
   claims it).
2. §2 in one commit.
3. §3 in one commit.

Each commit: full §3 bar first (fmt, clippy -D warnings, test
--workspace --locked, check-invariants.py — which must stay at
8 invariants / 30 bindings, unchanged). Push after the last, wait in
the FOREGROUND (`gh run watch <id> --exit-status`) for 8/8 green.

## Mutation pass — run after commit 1, before push

- the bang threshold flipped to `>` (80% loses its bang) — must be
  caught by a named test
- the sentinel fields (cwd / model / session id) routed into the output
  — caught
- `rate_limits` absent made a non-zero exit — caught
- `--statusline` allowed to combine with `--fail-at` — caught
- the session/weekly segment order swapped — caught

Any survivor: make it testable, fix it, same commit discipline as M16b.

## Report

`reports/m18a-endgate.md`, committed alone as commit 4: the pinned
stdin schema and where it came from; what landed per section; every
deviation numbered with reasoning; the mutation table; the §3 bar with
the new test count; CI run id and timestamps; winget validation output
or the honest note that it couldn't run; "things I was unsure of."
Do not self-accept (§4.8). The statusline's look in a real Claude Code
session is the owner's to judge (§4.5-adjacent: it renders in HIS
terminal).

## DO NOT

Touch any §4.1 path. Add a dependency. Bump the version. Write a
CHANGELOG entry. Submit anything to microsoft/winget-pkgs. Configure or
modify the owner's own Claude Code settings, statusline, or anything
under `~/.claude/**` (§4.4 — pin the schema from documentation, not
from the owner's live config). Start M18b. Use
`--dangerously-skip-permissions`.

## Housekeeping

The repo mount refuses `unlink`: after every git operation sweep
`.git/*.lock`, `.git/objects/maintenance.lock`,
`.git/objects/*/tmp_obj_*` into `_to_delete/git-stale/` with `mv`, then
verify `.git` is clean.
