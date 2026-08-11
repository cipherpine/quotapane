# Goal prompt spec: M18b — the update check, on invariant 5's own terms

Authored at the standing top tier 2026-08-11. Owner decisions (2026-08-11):
opt-in with a one-line first-run ask in the pane; the notify line is
display-only; scope is the update check plus the two M18a rulings.
Homebrew/AUR are NOT in scope.

Model tier: top (attended CLI session — §4a patches land protected bytes,
authored below; you verify and apply, never retype).
Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

## PRECONDITIONS (mismatch = STOP)

P1 Tip subject "packaging: the README moves out of the manifest directory
   it was breaking" (`f5ce9f6`) or this spec's own commit directly on top
   of it. Clean tree. Version 1.7.0. 502 tests (cli 106 + cli-integration
   13 + core 156 + ui 240 — count #[test] across crates if unsure).
P2 `update_check` appears nowhere in the workspace. `update.rs` does not
   exist under crates/usage-core/src/.

## Why this shape (context, not instructions)

SECURITY.md invariant 5 pre-committed the terms: notify-only, off by
default, docs change in the same PR. The egress module's own comment
pre-committed the other half: api.github.com returns to the allowlist
ONLY together with the code that calls it. Both promises come due here,
in one commit, under the same-change rule.

## §0 — the two M18a rulings (one commit, first)

0.1 `--client-version` joins `--statusline`'s conflict list (M18a D3
    ruling). Same error style as the existing conflicts, plus a test.
0.2 `format_reset`-for-statusline renders days above 48h — `resets 3d0h`,
    matching the window's own `resets in 5d 17h` habit rather than
    `resets 72h0m` (M18a §8.1 ruling). Pin with a table test. The
    past-reset behaviour (drop the segment) stays exactly as shipped.

## §1 — `usage_core::update` (new module, unprotected path)

One public function, roughly
`check(egress: &Egress) -> Option<UpdateNotice>`, where `UpdateNotice`
carries the newer version string and the releases URL constant. Rules:

- `GET https://api.github.com/repos/cipherpine/quotapane/releases/latest`
  through the existing egress chokepoint. Static
  `User-Agent: quotapane-update-check` — no version string, no OS, no
  identifier. GitHub rejects UA-less requests, so this is the minimum
  disclosure that works, and the report should say exactly that.
- **No token parameter exists in the module's API.** Nothing in
  `update.rs` can name, receive, or attach a credential. Three tests,
  all tagged `// INV:5` and registered in the manifest by Patch I:
  - `the_update_check_sends_nothing_unless_asked` — the gate: with the
    config off or absent, no request is constructed (structural: the
    window/CLI callers pass through one gate function; test it directly).
  - `the_update_request_cannot_carry_a_credential` — the module's source
    contains no `Authorization`, `Bearer`, `Secret`, or `token` token
    (comment-stripped scan, same pattern as the statusline module's
    self-test), and the request builder takes no such parameter.
  - `update_is_the_only_caller_of_the_github_host` — exactly one
    expression in the workspace names `api.github.com` outside
    `egress/mod.rs` and tests, and it is in `update.rs` (same
    single-call-site pattern as `refresh_agents`).
- Response handling: read at most 64 KiB of body; extract `tag_name`
  ONLY (allowlist-of-one; everything else in that JSON — release notes,
  asset URLs, author — is never deserialized into anything). Parse
  `vX.Y.Z`, compare numerically against `CARGO_PKG_VERSION`. Strictly
  newer → `Some(UpdateNotice)`. Equal, older, unparseable, HTTP error,
  network error → `None`. **No error type escapes this module** — a
  failed check is indistinguishable from no-update, by design.
- No caching, no state, no file writes. Once per window launch; the CLI
  variant runs when invoked. A window left running for days re-checks
  never — restart is the re-check, and the report records that as a
  deliberate simplicity, not an oversight.

## §2 — the window (usage-ui)

2.1 `config.cfg` grows `update_check` = `on` | `off`, default **absent**.
    Absent means un-asked. The config writer treats it like `theme` —
    written once on user choice, never on a timer.
2.2 First-run ask: when the key is absent, the usage view's footer (above
    the grip, the `// N older today` register and ink) shows ONE line:

        // check github for new versions?  on · off

    `on` and `off` are clickable words styled exactly like the
    `usage // agents` switcher (selectable(false), Sense::click(),
    installed over the drag handle the same way). Click writes the key
    and the line disappears this frame. It never returns unless the key
    is deleted by hand. The agents view never shows it.
2.3 When `update_check=on`: one `update::check` per launch, off the UI
    thread exactly like polls, result stored once. If newer:

        // v1.8.0 available

    faint (`TEXT_FAINT` / `weak_text_color`), display-only — no click
    handler, hover tooltip shows
    `github.com/cipherpine/quotapane/releases`. Amber is quota's colour
    and this line never uses it. `None` renders nothing, including on
    failure.
2.4 `--pace-demo` / `--agents-demo` never check (demos poll nothing) and
    never show the ask (a fixture is not a first run) — extend the
    existing demo-gating tests.
2.5 UI tests: the ask appears only when the key is absent; clicking
    each word writes the right value and removes the line; the notify
    line renders for `Some` and nothing for `None`; sentinel discipline —
    the notice's version string is the only new text that can reach the
    screen; a drag starting on the ask's words reaches the window handle
    (the switcher's gesture harness, reused).

## §3 — the CLI

`quotapane-cli --check-update`: a fourth mode, conflicts with `--once`,
`--watch`, `--statusline`, and the rest of the statusline conflict set.
Ignores `config.cfg` (an explicit command IS the opt-in). Prints either
`quotapane 1.7.0 — v1.8.0 available: github.com/cipherpine/quotapane/releases`
or `quotapane 1.7.0 — up to date`, and on a failed check says
`update check failed` and exits 1 (the CLI is allowed to be honest about
failure; only the window must be silent). Tests for all three outcomes
and the conflicts.

## §4 — docs (README config table row for `update_check`; a sentence in
the README's update-check paragraph in Security posture is handled by
Patch J; `docs/gating.md` untouched).

## §4a — protected-path patches (verify and apply, NEVER retype)

Extract every OLD/NEW programmatically from THIS FILE's bytes. Each OLD
must match exactly once before, each NEW exactly once after. All were
pre-flighted at the top tier 2026-08-11 against f5ce9f6.

### Patch A -> SECURITY.md (invariant 1: the new config key)

OLD:
```
a handful of key=value preference lines (theme, history, alerts — see the README)
```

NEW:
```
a handful of key=value preference lines (theme, history, alerts, update check — see the README)
```

### Patch B -> SECURITY.md (invariant 5 rewritten, its escape clause discharged)

OLD:
```
5. **No self-update — no update mechanism exists at all.** The app never downloads or executes code, and contains no updater and no update check of any kind. Updating is always a manual act: your package manager, or a verified download (see below). If an update *check* is ever added it will be notify-only and off by default, and this document changes in the same PR.
```

NEW:
```
5. **No self-update — and the update check is notify-only, off by default.** The app never downloads or executes code: there is no updater, and updating is always a manual act — your package manager, or a verified download (see below). An update *check* now exists on exactly the terms this document pre-committed to. It runs only when `update_check=on` in `config.cfg` (the window asks once, in one footer line, and records your answer; absent or `off` sends nothing), at most once per launch — or when you run `quotapane-cli --check-update` yourself, which is its own opt-in. The check is one anonymous GET to `api.github.com` for the latest release tag: no token (a test proves the request cannot carry one), no version string, no identifier beyond a static User-Agent. Versions are compared locally and a newer one is one faint line of text; a failed check shows nothing. Tests pin the gate, the single call site, and the credential-free request.
```

### Patch C -> SECURITY.md (network policy: the host count)

OLD:
```
- Single HTTP chokepoint, compile-time deny-by-default allowlist of exactly **two hosts** (`crates/usage-core/src/egress/mod.rs`, `ALLOWED_HOSTS`):
```

NEW:
```
- Single HTTP chokepoint, compile-time deny-by-default allowlist of exactly **three hosts** (`crates/usage-core/src/egress/mod.rs`, `ALLOWED_HOSTS`) — the two providers, plus one host reachable only through the opt-in update check:
```

### Patch D -> SECURITY.md (network policy: the removed host returns)

OLD:
```
- `api.github.com` was removed from the allowlist 2026-07-27: it existed for an optional update check that was never implemented, and an allowlist should be exactly as wide as the code behind it.
```

NEW:
```
- `api.github.com` — the opt-in update check (invariant 5), and nothing else. It was removed from this list 2026-07-27 because the check did not exist and "an allowlist should be exactly as wide as the code behind it"; the same rule brings it back now that the code does. Reachable from exactly one call site (`usage-core::update`, pinned by test), only when `update_check=on` or under `quotapane-cli --check-update`, and the request is anonymous — no credential can be attached (pinned by test), no version string, a static User-Agent.
```

### Patch E -> SECURITY.md (the verify-egress recipe)

OLD:
```
3. **Verify egress once.** Run `quotapane-cli --once --json` behind a packet capture or host-firewall allowlist and confirm the only destinations are `api.anthropic.com` and `chatgpt.com`. (Yes, that exact command works — it is tested; `--once` is required because one-shot polling is the CLI's only mode.)
```

NEW:
```
3. **Verify egress once.** Run `quotapane-cli --once --json` behind a packet capture or host-firewall allowlist and confirm the only destinations are `api.anthropic.com` and `chatgpt.com`. (Yes, that exact command works — it is tested.) With the update check off or unanswered — the default — `api.github.com` never appears, and its absence under capture is itself a verification you can run; it appears only under `update_check=on` or `--check-update`, once, at launch.
```

### Patch F -> THREAT_MODEL.md (T-E2)

OLD:
```
- **T-E2 — Silent auto-update escalates into arbitrary code execution as the user.** *Mitigation:* invariant 5, in its strongest form — **no update mechanism exists at all**: no updater code path, no update check, nothing to misconfigure. The egress allowlist (two provider hosts) leaves a covert updater nowhere to call.
```

NEW:
```
- **T-E2 — Silent auto-update escalates into arbitrary code execution as the user.** *Mitigation:* invariant 5 — **no updater code path exists**: nothing is ever downloaded or executed. The update *check* is notify-only, off by default, and structurally incapable of escalating: it can carry no credential, reads only a version tag, renders one line of text, and is the sole caller of the one non-provider host on the egress allowlist — so a covert updater still has nowhere to call and nothing to call with.
```

### Patch G -> THREAT_MODEL.md (§9 row 3)

OLD:
```
| 3. Deny-by-default egress | `egress` (`ALLOWED_HOSTS`, exactly two hosts) | `non_allowlisted_host_is_rejected` (incl. removed hosts `api.openai.com`, `api.github.com`), `get_refuses_non_allowlisted_host`, `authority_smuggling_paths_are_rejected` |
```

NEW:
```
| 3. Deny-by-default egress | `egress` (`ALLOWED_HOSTS`, exactly three hosts) | `non_allowlisted_host_is_rejected` (incl. the removed host `api.openai.com` and smuggling variants of all three allowed hosts), `get_refuses_non_allowlisted_host`, `authority_smuggling_paths_are_rejected` |
```

### Patch H -> THREAT_MODEL.md (§9 row 5)

OLD:
```
| 5. No self-update | absence of any updater code path | enforced by absence; the two-host allowlist test above doubles as the check that a covert updater has nowhere to call |
```

NEW:
```
| 5. No self-update; the check is notify-only, off by default | no updater code path (still enforced by absence) + `update` (gate, single call site, anonymous request) | `the_update_check_sends_nothing_unless_asked`, `the_update_request_cannot_carry_a_credential`, `update_is_the_only_caller_of_the_github_host` |
```

### Patch I -> invariants.manifest (invariants 3 and 5)

OLD:
```
invariant 3: Deny-by-default egress — compile-time two-host allowlist; a request to any other host is a hard error.
```

NEW:
```
invariant 3: Deny-by-default egress — compile-time three-host allowlist; a request to any other host is a hard error.
```

OLD:
```
invariant 5: No self-update — no updater or update check exists in the codebase at all.
kind: absence
enforced-by: absence of any updater code path; the invariant-3 allowlist leaves a covert updater nowhere to call
test: crates/usage-core/src/egress/mod.rs::non_allowlisted_host_is_rejected
```

NEW:
```
invariant 5: No self-update — nothing is downloaded or executed; the update check is notify-only, off by default, and credential-free.
kind: test-backed
test: crates/usage-core/src/update.rs::the_update_check_sends_nothing_unless_asked
test: crates/usage-core/src/update.rs::the_update_request_cannot_carry_a_credential
test: crates/usage-core/src/update.rs::update_is_the_only_caller_of_the_github_host
test: crates/usage-core/src/egress/mod.rs::non_allowlisted_host_is_rejected
```

### Patch J -> README.md (security bullet)

OLD:
```
- Network egress is deny-by-default through a single chokepoint with a compile-time allowlist of exactly **two hosts** (`api.anthropic.com`, `chatgpt.com`). Anything else is a hard error, and tests prove it.
```

NEW:
```
- Network egress is deny-by-default through a single chokepoint with a compile-time allowlist of exactly **three hosts**: the two providers (`api.anthropic.com`, `chatgpt.com`), plus `api.github.com`, reachable only through the opt-in, notify-only update check. Anything else is a hard error, and tests prove it.
```

### Patch K -> crates/usage-core/src/egress/mod.rs (the host returns)

OLD:
```
    "chatgpt.com",
    // api.github.com was REMOVED 2026-07-27 (M6): it existed for the optional
    // release update *check* (invariant 5), which is not implemented — zero
    // callers in the workspace (gap report). Re-add it ONLY together with the
    // update-check code itself, per the rules above.
];
```

NEW:
```
    "chatgpt.com",
    // GitHub Releases: the opt-in, notify-only update check (invariant 5).
    // Removed 2026-07-27 when it had zero callers; returned together with
    // `usage-core::update`, its exactly-one caller, per this comment's own
    // rule. That module's tests pin the single call site and prove the
    // request cannot carry a credential; the on/off gate lives with the
    // caller, so this host is reached only when `update_check=on` or under
    // `quotapane-cli --check-update`.
    "api.github.com",
];
```

### Patch L -> crates/usage-core/src/egress/mod.rs (rejection fixtures)

OLD:
```
            "openai.com",           // bare apex is NOT allowlisted
            "api.openai.com",       // withdrawn with M4 (ADR-002) — no longer allowlisted
            "api.github.com",       // removed 2026-07-27 — update check unimplemented, zero callers
            "localhost",
```

NEW:
```
            "openai.com",           // bare apex is NOT allowlisted
            "api.openai.com",       // withdrawn with M4 (ADR-002) — no longer allowlisted
            "github.com",              // bare apex is NOT allowlisted — only the API host is
            "evil.api.github.com",     // subdomain of the update-check host
            "api.github.com.evil.com", // update-check host as a prefix label
            "api.github.com:8443",     // port smuggling on the update-check host
            "localhost",
```

## Commits

1. §0 — the two cleanups.
2. §1 + §2 + §3 + §4 + ALL §4a patches in ONE commit: the update check,
   its gate, its docs, and every claim they change (the same-change rule;
   the checker's set-equality forces the manifest half anyway).
3. The end-gate report.

Full §3 bar before each commit. `tools/check-invariants.py` must report
**8 invariants and set-equality** after commit 2 (binding count will rise;
record the new number). Push after commit 3, wait FOREGROUND
(`gh run watch <id> --exit-status`) for 8/8 green. Note: rustfmt may
reflow Patch K/L comment lines — M16's D2 precedent applies: take the
formatter, flag the reflow, never `#[rustfmt::skip]`.

## Mutation pass — after commit 2, before push. Run with --no-fail-fast
and byte-identical revert checks (the M18a CRLF lesson).

- the gate inverted (checks when off) — caught
- the gate removed (checks when absent/un-asked) — caught
- `tag_name` parse widened to read the release `body` — caught
- an `Authorization` header added to the update request — caught by the
  source-scan test
- a second call site for api.github.com added elsewhere — caught
- the notify line rendered on `None` — caught
- the first-run ask shown when the key is present — caught
- `--check-update` combined with `--once` allowed — caught
- version compare flipped (older reported as newer) — caught
- the ask rendered in a demo — caught

## Report

`reports/m18b-endgate.md`: what landed per section; the §4a table (every
patch OLD×1 before / NEW×1 after, byte-verified); deviations numbered;
the mutation table; the §3 bar with new test and binding counts; CI run
id and timestamps; things you were unsure of. The first-run ask and the
notify line are §4.5 — the owner's eyes, never self-accepted. No release
is cut; version stays 1.7.0.

## DO NOT

Retype a protected byte (extract from this file programmatically). Add a
dependency. Bump the version. Write a CHANGELOG entry. Cache anything or
write any new file at runtime. Give the notify line a click handler. Send
any version string, OS, or identifier in the update request. Touch
`.github/**`, `assets/**`, or `deny.toml`. Read `~/.claude/**` or
`~/.codex/**`. Start Homebrew/AUR. Use `--dangerously-skip-permissions`.

## Housekeeping

Sweep `.git/*.lock`, `.git/objects/maintenance.lock`,
`.git/objects/*/tmp_obj_*` into `_to_delete/git-stale/` with `mv` after
every git operation; verify `.git` clean.
