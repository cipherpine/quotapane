# M10 — the expired-token experience explains itself (ships as v1.4.1)

Authored at the top tier, 2026-07-31. Owner decision: ship now as a
dedicated patch slice. Executes on the floor per DECISIONS.md §4a
discipline. Read DECISIONS.md before touching anything.

## Why

TokenExpired is the most-seen error state in the product: Claude access
tokens expire on the order of hours, so every user who does not live in
the CLI hits this screen regularly. Today the message asks the user to
already understand the architecture ("refresh via the provider's
official CLI") to know what to do. The owner — who built the thing —
found the recovery non-obvious. Fix: the message names the exact
command, per provider, and says that QuotaPane recovers on its own.

Two adjacent nits ship in the same slice: the at-risk pace line
currently renders off stale data (a forecast extrapolated from dead
data is misinformation), and the README has no answer to "why does it
say my token expired?".

## Boundaries (read twice)

- **Zero new dependencies.** A needed crate is a STOP + report, not an ADR
  written by you.
- **No §4.1 path may change**: `crates/usage-core/src/egress/**`,
  `crates/usage-core/src/credentials/**`, any security-invariant test,
  `deny.toml`, `SECURITY.md`, `THREAT_MODEL.md`, `.github/**`,
  `.cargo/**`, `.claude/**`. If any phase appears to require touching
  one, STOP and report (§4.7). This slice was scoped so none does.
- **No version bump, no CHANGELOG edit.** Both belong to M10-RELEASE.
- Every user-visible string below is **byte-exact**. Do not reword, do
  not "improve", do not change punctuation. `—` is U+2014, `~` is
  ASCII tilde, backticks are literal backticks.
- §4.4 stands: no token material in any message, log, or test fixture.

## Phase 1 — the message names the action (one commit)

### 1a. Core fallback (`crates/usage-core/src/providers/mod.rs`)

`ProviderError::TokenExpired`'s `Display` arm becomes exactly:

    OAuth token expired — refresh it in the provider's official CLI (start a `claude` session, or run `codex login`); QuotaPane retries automatically

The `OAuth token expired` prefix is now a **stable marker** the UI keys
on (same pattern as `is_absent_credentials`); the doc comment on the
`Display` impl must say so, so nobody renames it casually.

### 1b. UI per-pane copy (`crates/usage-ui/src/main.rs`)

- Add a `FailureDisplay::TokenExpired` variant. `classify_failure`
  returns it when the message starts with `OAuth token expired` —
  checked **after** the `NotSignedIn` arm, before `Banner`.
- Add `fn token_expired_line(id: ProviderId) -> &'static str` (same
  shape and placement as `not_signed_in_line`), returning exactly:
  - Claude pane:

        token expired — start any claude session (even `claude -p hi`) to refresh it. QuotaPane recovers on its own within ~3 min.

  - Codex pane:

        token expired — run `codex login` to refresh it. QuotaPane recovers on its own within ~3 min.

- In `render_pane`, the `TokenExpired` arm renders
  `token_expired_line(pane.id)` as a CARDINAL colored label — the raw
  poller message is not shown in this case.

### 1c. CLI hint (`crates/usage-cli/src/main.rs`)

`report_provider_error` already holds the structured `&ProviderError`
and the provider id. When the error matches
`ProviderError::TokenExpired`, print the error line as today, then one
additional stderr line, exactly:

- claude:

      hint: start any claude session (even `claude -p hi`) to refresh the token, then rerun

- codex:

      hint: run `codex login` to refresh the token, then rerun

Factor the hint text into a pure function over the provider id so it is
unit-testable. Follow the existing proxy-hint pattern for how hints
print.

### 1d. Tests (Phase 1)

- **Pin test (the load-bearing one):** in usage-ui,
  `classify_failure(Some(&ProviderError::TokenExpired.to_string()))`
  is `FailureDisplay::TokenExpired`. This welds the core marker to the
  UI matcher — if either drifts, CI fails.
- `token_expired_line` returns the exact strings above for both ids
  (assert full string equality, not `contains`).
- The CLI hint function returns the exact strings above for both ids.
- Existing `Display` tests for `TokenExpired` update to the new text.
  If any test you must touch is a security-invariant test, STOP.
- `classify_failure` existing cases (`NoFailure`, `NotSignedIn`,
  `Banner`) still pass unchanged.

Commit message: `ui,cli: expired-token message names the exact refresh action`

## Phase 2 — the at-risk line goes quiet when data is stale (one commit)

In `render_windows` (usage-ui), the pace warning currently draws
whenever `pace` is `Some`. Gate it: draw only when
`!age.is_some_and(is_stale)` — the same staleness predicate and the
same `age.is_some_and(..)` shape the footer already uses. `age == None`
keeps drawing (consistent with existing treatment elsewhere).

Do **not** touch the pace tick, `select_pace_warning`, `pace::at_risk`,
or anything in `usage_core::pace` — the forecast is still computed and
stored; only its rendering is suppressed while stale. When fresh data
arrives the line simply reappears.

Factor the decision into a pure helper (e.g.
`fn show_pace_warning(age: Option<Duration>) -> bool`) and unit-test
it: fresh age → true; age at exactly the staleness threshold → matches
`is_stale`'s boundary behavior; well past threshold → false; `None` →
true. Existing pace tests must pass untouched.

Commit message: `ui: suppress the at-risk pace line once data is stale`

## Phase 3 — README FAQ (one commit)

Add to `README.md`. If a FAQ or Troubleshooting section exists, add
this as a subsection there; otherwise create a `## FAQ` section
immediately before the Disclaimer section. Verbatim:

    ### Why does it say my token expired?

    QuotaPane has no login of its own — it reads the credential files the
    official `claude` / `codex` CLIs keep on your machine, and it never
    writes them. When the stored token's lifetime runs out, QuotaPane fails
    closed: it stops sending the stale token and shows the message instead.

    The refresh happens in the provider's CLI, not in QuotaPane:

    - **Claude** — start any `claude` session (even `claude -p hi`). The
      CLI refreshes its token file as it starts working.
    - **Codex** — run `codex login`.

    That's all. QuotaPane rechecks every 3 minutes while a token is
    expired and recovers on its own — no restart, no clicks.

Commit message: `docs: FAQ — why does it say my token expired`

## The bar (§3), every phase

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, full
`cargo test` — all green **before** each commit. Zero warnings.

## End gate

Push all three commits, wait for CI (all 7 checks green), then report:
the three SHAs, test count delta, confirmation that each exact string
above appears verbatim (quote your grep), confirmation that no §4.1
path changed (`git diff --stat` against the pre-M10 HEAD), and STOP.
Acceptance is the owner's (§4.8). No version bump, no tag, no release —
M10-RELEASE is a separate spec.
