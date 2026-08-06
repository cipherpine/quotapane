# M15 — the agents pane: who is working right now

**Status:** speced 2026-08-06 (top tier). Owner decisions 2026-08-06: a
`usage // agents` TAB in the main window (pop-out deferred); identity is
CONTENT-FREE ONLY (`project · branch · short-id` — nothing from inside a
conversation is ever read or rendered). Direction is the B2 local-log
analytics idea the owner raised on 2026-08-04 ("see in flight jobs").

## Research findings this spec is built on (verified at the top tier)

- **Claude Code** writes one append-only JSONL per session at
  `~/.claude/projects/<encoded-cwd>/<session-id>.jsonl`. Every line carries
  content-free metadata alongside its payload: `sessionId`, `timestamp`
  (ISO 8601), `type`, `cwd`, `gitBranch`, `version`, `uuid`, `parentUuid`.
  Anthropic's own session-browser cookbook enumerates sessions with file
  stats plus head/tail slices — no full parse.
- **Codex CLI** writes `~/.codex/sessions/YYYY/MM/DD/rollout-<ts>-<id>.jsonl`
  ($CODEX_HOME honored, as the credentials module already does). First line
  is a `session_meta` record: `id`, `source` (originator), `cwd`,
  `model_provider`, `cli_version`, git fields. Later lines are timestamped
  event records.
- **Neither format marks session end.** Liveness must be inferred: a file
  recently written is a session actively working. The tail line's record
  type refines the state; the mtime alone is a sufficient fallback.
- These are UNDOCUMENTED INTERNAL formats that drift with CLI versions.
  The parser must be tolerant by construction: unknown types ignored,
  missing keys tolerated, a file that fails to parse still reports
  liveness from its mtime. This is the same posture docs/cli-json.md
  demands of our own consumers.

## What this is NOT

No network. No new files written (invariant 1's two-file claim is
untouched — agents data is in-memory only). No `--json` change in this
slice (a `--json` agents block is a future, separately-speced decision).
No process enumeration. No reading of `.credentials.json` / `auth.json`
beyond what the credentials module already does.

---

## Phase 1 — `usage_core::agents` + the invariant, one commit

### The module

`crates/usage-core/src/agents.rs` — scanner + tolerant metadata parser.

- `AgentSession { provider: ProviderId, short_id: String /* first 8 */,
  project: String /* basename of cwd */, branch: Option<String>,
  state: AgentState, last_write: SystemTime, age: Duration }`
- `AgentState { Working, Idle, Recent }` from mtime against consts, each
  with a doc comment naming its rationale:
  `ACTIVE_WITHIN: 120s` (a session mid-turn writes far more often than
  this), `IDLE_WITHIN: 1800s`, `LOOKBACK: 24h` (older files are not even
  opened). A test asserts ACTIVE_WITHIN < IDLE_WITHIN < LOOKBACK.
- **Scan:** enumerate `<claude_root>/projects/*/*.jsonl` and
  `<codex_root>/sessions/*/*/*/rollout-*.jsonl`; stat first; only files
  with mtime inside LOOKBACK are opened; open = read the FIRST line and
  the LAST ≤16 KiB (`TAIL_CAP` const), nothing between. Roots are
  parameters (dependency-injected paths, exactly how credentials tests
  point at temp dirs) — production wiring resolves `~/.claude` and
  `$CODEX_HOME`-or-`~/.codex`.
- **Allowlisted keys — the whole list, a const the tests weld to:**
  Claude: `sessionId`, `timestamp`, `type`, `cwd`, `gitBranch`,
  `isSidechain` (best-effort; when present-and-true the row is marked as a
  subagent). Codex `session_meta`: `id`, `cwd`, `git_branch`, plus the
  wrapper `timestamp`/`type` of the tail line. Extraction is key-by-key
  from the parsed JSON value; the message/content payload is NEVER
  deserialized into any output type, NEVER logged, NEVER stored.
- **Fixtures:** synthetic, inline in the test module (the history-module
  idiom). Every fixture line's content fields carry the sentinel string
  `SENTINEL-DO-NOT-SURFACE`; tests assert the sentinel is unreachable from
  every public type's Debug/Display and every field.

### The invariant — §4a patches, THIS COMMIT (same-change rule; the
### checker's tag↔manifest set-equality makes any other sequencing red)

Tag the four tests below `// INV:8`. All OLD/NEW blocks are to be
extracted programmatically from THIS FILE's bytes, proven unique before
and after, per §4a. Four patches, four files, one commit with the module.

**Patch A — SECURITY.md, invariant list.** OLD (unique):

```
7. **Proxy is opt-in, CLI-only, and fail-closed.** If a proxy environment variable is set (`HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY`, upper- or lowercase), egress sends nothing and fails with an error naming the variable. `quotapane-cli --allow-proxy` opts in for that single run, after a printed warning that a TLS-inspecting proxy can observe the bearer token; the CLI's error output points at the flag. The window has no opt-in surface at all — its egress is constructed proxy-off unconditionally, so under a proxy environment it fails closed and shows the error rather than sending anything anywhere.
```

NEW:

```
7. **Proxy is opt-in, CLI-only, and fail-closed.** If a proxy environment variable is set (`HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY`, upper- or lowercase), egress sends nothing and fails with an error naming the variable. `quotapane-cli --allow-proxy` opts in for that single run, after a printed warning that a TLS-inspecting proxy can observe the bearer token; the CLI's error output points at the flag. The window has no opt-in surface at all — its egress is constructed proxy-off unconditionally, so under a proxy environment it fails closed and shows the error rather than sending anything anywhere.
8. **Agent visibility is metadata-only.** The agents pane lists your local Claude Code / Codex CLI sessions by reading their session-log files (`~/.claude/projects/`, `~/.codex/sessions/`) read-only, extracting a fixed allowlist of metadata keys — ids, timestamps, record types, working directory, git branch — and nothing else. Conversation content is never deserialized, rendered, persisted, or transmitted; a fixture test plants sentinel conversation text and asserts it cannot reach any output. Liveness is inferred from file modification times alone when parsing fails, so degradation is graceful and silent. Nothing about these sessions leaves the machine.
```

**Patch B — SECURITY.md, credential handling.** OLD (unique):

```
- Sources are read-only: `~/.claude/.credentials.json`, and `$CODEX_HOME/auth.json` (or `~/.codex/auth.json`). Nothing else is read — WSL credential sources are **not** implemented (a possible post-1.0 addition, which would be called out here).
```

NEW:

```
- Sources are read-only: `~/.claude/.credentials.json`, and `$CODEX_HOME/auth.json` (or `~/.codex/auth.json`). No other credential source is read — WSL credential sources are **not** implemented (a possible post-1.0 addition, which would be called out here). The only other provider files the app opens are the session-log files named by invariant 8: read-only, metadata-only, never a token-bearing path.
```

**Patch C — invariants.manifest.** OLD (unique):

```
test: crates/usage-cli/tests/cli.rs::allow_proxy_prints_the_token_visibility_warning_and_passes_the_gate
```

NEW:

```
test: crates/usage-cli/tests/cli.rs::allow_proxy_prints_the_token_visibility_warning_and_passes_the_gate

invariant 8: Agent visibility is metadata-only — the agents pane reads session-log files read-only, extracts a fixed allowlist of metadata keys, and never deserializes, renders, persists, or transmits conversation content; liveness degrades gracefully to file mtimes.
kind: test-backed
test: crates/usage-core/src/agents.rs::sentinel_content_never_reaches_any_output
test: crates/usage-core/src/agents.rs::extraction_is_welded_to_the_allowlist_const
test: crates/usage-core/src/agents.rs::unparseable_file_still_reports_liveness_from_mtime
test: crates/usage-core/src/agents.rs::scanner_opens_only_jsonl_under_the_session_roots
```

**Patch D — THREAT_MODEL.md, §9 table.** OLD (unique):

```
| 7. Proxy opt-in (CLI-only, fail-closed) | `egress` proxy gate; `quotapane-cli --allow-proxy` is the only opt-in surface — the window has none | `proxy_env_without_opt_in_fails_closed` (either casing) + opt-in and empty-var tests; CLI tests pin the hint line, the per-run warning, and the absence of a window opt-in |
```

NEW:

```
| 7. Proxy opt-in (CLI-only, fail-closed) | `egress` proxy gate; `quotapane-cli --allow-proxy` is the only opt-in surface — the window has none | `proxy_env_without_opt_in_fails_closed` (either casing) + opt-in and empty-var tests; CLI tests pin the hint line, the per-run warning, and the absence of a window opt-in |
| 8. Agent visibility is metadata-only | `usage_core::agents` — allowlisted key extraction; content payloads never deserialized | `sentinel_content_never_reaches_any_output`, `extraction_is_welded_to_the_allowlist_const`, `unparseable_file_still_reports_liveness_from_mtime`, `scanner_opens_only_jsonl_under_the_session_roots` |
```

**Patch E — THREAT_MODEL.md, threat list.** OLD (unique):

```
- **T-I5 — Token observed by a TLS-inspecting proxy.** *Mitigation:* invariant 7 — proxy off by default, explicit warning + opt-in. *Residual:* R3.
```

NEW:

```
- **T-I5 — Token observed by a TLS-inspecting proxy.** *Mitigation:* invariant 7 — proxy off by default, explicit warning + opt-in. *Residual:* R3.
- **T-I6 — Conversation content surfaced or persisted by the agents pane.** *Mitigation:* invariant 8 — allowlisted metadata keys only; the sentinel-content test proves the content payload cannot reach any output type; nothing is written to disk or sent anywhere.
```

### Tests (Phase 1, beyond the four INV:8 bindings)

- State thresholds welded to the consts (boundary table, no literals).
- Claude fixture, Codex fixture, a mixed tree, an empty tree, a file with
  a garbage first line (→ mtime-only `Recent`/`Working` row with project
  from the directory name), a file outside LOOKBACK (→ never opened —
  prove via a fixture file that is not valid UTF-8, which would error if
  read).
- `isSidechain: true` marks a subagent row; absence of the key does not.
- Codex date-tree enumeration crosses a month boundary correctly.

---

## Phase 2 — the tab, one commit

- Titlebar gains the view switcher in the M7b terminal voice:
  `usage // agents` (current view TEXT, other TEXT_FAINT, click to
  switch). The switcher must coexist with StartDrag (main.rs:1695) — a
  click that lands on a label switches, a drag on the rest of the bar
  still moves the window. Default view: usage. Not persisted.
- Agents view, same 320px pane, same ScrollArea + grip chrome as M14:
  rows grouped under the existing provider header style. Row =
  state dot (OPER_GREEN Working / AMBER Idle / TEXT_FAINT Recent — the
  M14 freshness-dot palette semantics, deliberately rhymed),
  `project · branch · id8` (branch omitted when None), subagent rows
  prefixed `· sub`, right-aligned age via format_age. Empty state:
  `// no agent sessions in the last 24h` in TEXT_FAINT.
- Scanning: only while the agents view is showing — a scan on switch,
  then every `SCAN_EVERY: 2s` via request_repaint_after. No scanning,
  none at all, while the usage view shows (no badge in v1; that is a
  future decision, not a cheap default).
- `--agents-demo` flag: the agents view renders a synthetic fixture set
  (one Working with subagent, one Idle, one Recent, both providers) for
  the owner's §4.5 pass — the --pace-demo idiom exactly; demo never
  touches real roots.
- Doc touch, same commit: README gains a short "Agents view" paragraph
  (content-free identity, local-only, invariant 8 link) — README is not
  §4.1; write it plainly.

### Tests (Phase 2)

- Switcher hit-test: label click switches, bar drag does not switch.
- The laid-out agents pane never contains the sentinel (fixture wired
  through the real render path via the harness).
- Demo fixture renders all three states + the subagent marker (harness).
- No scan call is reachable while the usage view shows (guard unit test).

---

## The bar (§3, every push)

cargo fmt --all --check · cargo clippy --workspace --all-targets --locked
-- -D warnings · cargo test --workspace --locked ·
python3 tools/check-invariants.py. Phase 2 only after Phase 1's CI is
green on all 8 required checks. FOREGROUND waits only
(`gh run watch <id> --exit-status`); never background watchers.

## Hard limits

Zero new dependencies. No `--json` change. No version bump, no CHANGELOG
(M15-RELEASE's job). §4.1: the five patch blocks above are the ONLY
protected-path bytes you may change, applied byte-exactly under §4a — any
other need is a STOP per §4.7. Never read the REAL `~/.claude/**` or
`~/.codex/**` yourself (§4.4) — the product reads them at the USER'S
runtime; your tests read fixtures in temp dirs, and `--agents-demo` is
how the owner sees it live without you touching his logs. Never print,
log, or persist anything a fixture marks as content. §4.5: you never
accept visuals.

## End gate

`reports/m15-endgate.md` on main, CI green on both phase commits (each
waited in the foreground), the reports/README.md convention, mutation
checks proving the sentinel test and the allowlist weld actually bite
(delete a key from the allowlist const → a test must fail; route one
content field into a pub field → the sentinel test must fail), and the
§4.5 items listed for the owner's pass. EXIT after pushing; nothing
further is queued from your side.
