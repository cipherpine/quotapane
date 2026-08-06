# M16 — the agents pane, second pass

Two phases, two commits, then a report. Charter rules as always: `DECISIONS.md`
§3 is the bar, §4a governs every byte in a protected file, §4.4 governs what you
may read, §4.6 governs what you do when CI disagrees with you, §4.7 governs what
you do when this spec disagrees with the tree.

**Read this whole file before you touch anything.** Phase 1 changes a security
surface, and the reasoning for why it is safe is in here, not in the diff.

---

## Why

The owner has been living with the M15 agents pane. Two verdicts, both fair:

1. **It is full of the dead.** A 24 h lookback means a morning's finished
   sessions crowd out the one that is running now. The pane is a status light;
   it should open on what is alive.
2. **It does not say what anyone is doing.** A green dot and a project name tell
   you a process wrote a file recently. They do not tell you whether the agent
   is grinding through a tool loop or has been sitting for ten minutes waiting
   for a human to read its question.

He also confirmed the boundary: **strictly content-free, invariant 8 stands.**
Everything below gets its usefulness out of metadata or does not ship.

The one thing you must not do is solve (2) by reading more of the file. The
answers here come from record *types*, record *counts*, and record
*timestamps* — never from a record's payload.

---

## Phase 1 — `crates/usage-core/src/agents.rs`

### 1.1 Reach one level deeper, and build a fence while you are there

M15 shipped `read_allowlisted` with a two-level search: the record object and
its direct children. That was chosen so the Codex CLI's `payload`-wrapped
`session_meta` fields were reachable without naming `payload`.

It is one level short. Recent Codex builds nest the branch at
`payload.git.branch`, so the pane shows Codex rows with no branch at all (M15
deviation 3, still open — the owner has not confirmed either way, so make it
work both ways rather than picking one).

Change the search to three object levels. Name the depth:

```rust
/// How many levels of objects [`read_allowlisted`] will look through: the
/// record, its children, and their children.
///
/// Three, because the Codex CLI writes its branch at `payload.git.branch` while
/// writing its id at `payload.id`, and a search that reaches one but not the
/// other produces a row that is silently missing a field. Not four: depth is
/// reach, reach is risk, and nothing either CLI writes needs more.
const MAX_KEY_DEPTH: usize = 3;
```

Depth is reach, and reach is the thing that was keeping the allowlist honest.
A two-level search could not have found `model` inside a Claude `message`
object; a three-level search can, the day somebody adds `"model"` to the
allowlist in good faith. So the widening arrives with its own fence:

```rust
/// **Names that may never join [`ALLOWLISTED_KEYS`].**
///
/// The allowlist says what may be read. This says what may never be *added* —
/// the keys under which both CLIs file the actual words. It exists because
/// [`MAX_KEY_DEPTH`] is 3: the lookup can now reach inside a `message` object,
/// so the guarantee that it never returns a sentence stopped being a property
/// of the search and became a property of the list. A test welds the two
/// together, and a reviewer adding a key to one of these lists will be told by
/// CI if it is already on the other.
const FORBIDDEN_KEYS: &[&str] = &[
    "content",
    "text",
    "message",
    "summary",
    "instructions",
    "toolUseResult",
    "model",
    "input",
    "output",
    "command",
    "stdout",
    "stderr",
    "commit_message",
];
```

`model` is on that list deliberately, and it is the interesting entry. The owner
asked for model/CLI provenance on each row; a model slug is not content, and if
it lived beside the record we would read it. It does not — Claude Code writes it
at `message.model`, inside the one object this module exists to leave shut.
Provenance therefore comes from the CLI version instead (§1.4), and the reason
is written down here so the next person does not "fix" it.

Update `ALLOWLISTED_KEYS` — add `branch` (Codex's nested spelling), plus the two
version keys §1.4 needs — and update the const's doc comment to match, including
its sentence about the two-level search.

Final list, in the source's existing grouping:

```rust
pub const ALLOWLISTED_KEYS: &[&str] = &[
    // Claude Code record lines.
    "sessionId",
    "timestamp",
    "type",
    "cwd",
    "gitBranch",
    "isSidechain",
    "version",
    // Codex CLI `session_meta` payload.
    "id",
    "git_branch",
    "branch",
    "cli_version",
];
```

In `Extracted::absorb`, the Codex branch read tries `git_branch` first and
`branch` second, so a build that writes either spelling produces the same row.
Do not remove the flat spelling: older Codex logs on the owner's disk still use
it, and both are the same fact.

### 1.2 Turn state — whose move is it

The one genuinely new signal, and it costs nothing on the boundary because it
reads a record's `type` and stops.

```rust
/// Whose move it is in a session that is still going.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnState {
    /// The agent is mid-loop: the last thing written was input *to* it — a
    /// tool result, or a human's message it has not answered yet.
    InLoop,
    /// The agent has finished its turn and the human has not replied. This is
    /// the row worth walking back to your desk for.
    YourTurn,
    /// Not knowable from this session's records. Never a guess.
    Unknown,
}
```

The whole derivation, as one pure function so a test can hold it:

```rust
/// Whose move it is, from the last record's `type` and nothing else.
///
/// Claude Code alternates two record types on the main transcript: `assistant`
/// for what the model produced, `user` for what came back to it — which
/// includes tool results, not only typed messages. So a transcript ending in
/// `user` is an agent that has work in hand, and one ending in `assistant` is
/// an agent that has stopped and is waiting to be read.
///
/// Codex's record vocabulary does not draw that line in a way this module can
/// read without opening payloads, so every Codex row is [`TurnState::Unknown`]
/// and shows no turn phrase at all. A blank is the honest answer; a wrong
/// "your turn" on a session that is busy would be worse than the M15 pane.
///
/// A [`AgentState::Recent`] session is over. It has no turn.
pub fn turn_for(
    provider: ProviderId,
    record_type: Option<&str>,
    state: AgentState,
) -> TurnState
```

Only the **tail** record may set the turn. The head record is the session's
first line; what it says about turns is a day stale.

### 1.3 Clock arithmetic without a dependency

Both `duration` (§1.4) and `pulse` (§1.5) need a record's timestamp as a number.
Nothing in `usage-core` parses RFC 3339 today and **zero new dependencies** is
still the rule, so write one small strict reader:

```rust
/// The epoch seconds in an RFC 3339 stamp, or `None` for anything that is not
/// exactly the shape both CLIs write.
///
/// Strict on purpose: `YYYY-MM-DDTHH:MM:SS`, an optional fractional part which
/// is read and discarded, then either `Z` or `±HH:MM`, which is subtracted.
/// Anything else — a two-digit year, a missing zone, a month of 13 — is `None`,
/// and `None` costs a row its duration and its pulse while costing it nothing
/// else. This module already treats an unreadable file as a live session with
/// a thin row; an unreadable timestamp is the same kind of nothing.
fn epoch_secs(stamp: &str) -> Option<u64>
```

Use the days-from-civil conversion (the standard branch-free one); no leap
seconds, no local zone, no `chrono`. Test it directly: the epoch itself, a
fractional stamp, a `+05:30` offset, a `-08:00` offset, a leap day, a
month-boundary, and at least four malformed shapes that must all return `None`.

### 1.4 Duration and provenance

Two more fields on `AgentSession`, both cheap:

- `duration: Option<Duration>` — from the **head** record's `timestamp` to
  `last_write`. How long this session has been going. `None` when the head line
  yields no usable stamp, or when the arithmetic would run backwards (a clock
  change; saturate to `None` rather than to zero, which would read as a claim).
- `cli_version: Option<String>` — Claude Code stamps `version` on every record;
  Codex writes `cli_version` in `session_meta`. Read whichever the provider
  writes. Cap it at 16 characters when storing: it is a version string, and a
  field of unbounded length has no business in a 320 px row even truncated.

### 1.5 Pulse — the shape of the last ten minutes

The row's activity sparkline. Content-free by construction: it counts
timestamps.

```rust
/// How many one-minute buckets a row's pulse carries.
pub const PULSE_BUCKETS: usize = 10;
/// How long one pulse bucket covers.
pub const PULSE_BUCKET: Duration = Duration::from_secs(60);
/// The most any one bucket will count to. A busier minute than this is
/// indistinguishable from this one at 7 px tall.
pub const PULSE_CAP: u32 = 999;
```

`pulse: [u32; PULSE_BUCKETS]` on `AgentSession`, oldest bucket first, newest
last, covering the ten minutes ending at `now`.

It is filled from the tail chunk you already read. Two changes to make that
possible:

- `read_head_and_tail` returns `(String, Vec<String>)`: the head line, and
  **every complete line** in the tail chunk rather than only the last. The
  `TAIL_CAP` bound is unchanged, so the cost is unchanged — you are parsing
  bytes that were already in memory.
- `absorb` still sees only the last of those lines. Widening what is *read* must
  not widen what is *believed*.

For each tail line: parse, `is_record`, take `timestamp`, `epoch_secs`, bucket
it. Nothing else. A line that fails any step contributes nothing and is not an
error.

Compute the pulse **only** when `state != AgentState::Recent`. A finished
session's rhythm is not a thing anyone is watching, and skipping it keeps the
per-frame cost on the rows that matter. A `Recent` row's pulse is all zeros,
which the UI draws as nothing at all.

### 1.6 Tests

Everything above needs the module's usual coverage, plus these specifically:

- `epoch_secs` — the table in §1.3.
- The Codex branch, both spellings, through a full `scan`: two fixture rollouts,
  one flat `git_branch`, one nested `payload.git.branch`, both producing a row
  with the same branch.
- Pulse: a fixture whose tail holds a known spread of timestamps produces the
  exact expected bucket counts; a fixture with an unparseable tail produces all
  zeros *and still produces a row*; a `Recent` fixture's pulse is all zeros.
- Duration: head-to-mtime on a fixture; `None` when the head has no stamp;
  `None` rather than zero when the head stamp is *after* the mtime.
- Every new fixture in this module carries `SENTINEL-DO-NOT-SURFACE` in its
  content fields, like every fixture already there. The existing
  `sentinel_content_never_reaches_any_output` must cover the new fields —
  extend `every_output` so it formats `turn`, `duration`, `cli_version` and
  `pulse` too. **A new field that the sentinel test does not format is a hole
  in invariant 8.** That extension is yours to write; it is not a protected
  test's body, only its helper.

Two *new* protected tests and the changes to one existing one arrive as byte
patches in §4a below. Do not write them yourself; apply them.

---

## Phase 2 — `crates/usage-ui/src/main.rs`

### 2.1 Recent by default, older on demand

New presentation const, beside the other agents-view consts:

```rust
/// How recently a session must have been written to appear without asking.
///
/// Two hours. The pane's job is "who is working right now", and the 24 h
/// lookback — which is a scanning bound, and stays one — turned out to be a
/// reading list. Everything older is one line away, never gone.
const AGENTS_RECENT_WINDOW: Duration = Duration::from_secs(2 * 60 * 60);
```

Split the scanned list on `age <= AGENTS_RECENT_WINDOW`. Render the recent set
exactly as M15 renders sessions today — provider grouping, headers, order
untouched. Then, when the older set is non-empty, one dim clickable line at the
foot of the pane:

- collapsed: `// 7 older today`  (`1 older today` for one — no bare plural)
- expanded: `// hide older`

Clicking toggles `agents_show_older: bool` on the app. When expanded, the older
rows join their own provider groups above the line, dimmed to `TEXT_FAINT`
(CipherPine) / `weak_text_color` (Plain) so the split stays visible without a
second header.

**Not persisted**, for the reason `View` is not persisted, and the doc comment
should say so by pointing at that one.

Two empty states now, not one:

- Nothing scanned at all: the existing `NO_AGENTS_LINE`, unchanged.
- Something scanned, but all of it older than two hours, collapsed:
  `const NO_RECENT_AGENTS_LINE: &str = "// nothing active in the last 2h";`
  above the toggle line. Do not show the 24 h line here — it would be false.

`render_agents` grows a `show_older: &mut bool` parameter. Its callers and its
tests come with it.

### 2.2 The second line

A row becomes two lines when it has a second thing to say. Line one is exactly
what M15 draws — dot, identity, right-aligned age. Line two is small, dim, and
indented to sit under the identity rather than under the dot:

```
● QuotaPane · main · a1b2c3d4                    12s
  ▁▂▅█▇▃▁▁▁▁  in the loop · up 12m · v2.0.14
```

Rules:

- **Only for `Working` and `Idle` rows.** A `Recent` row is one line. This is
  what keeps the expanded older list from doubling in height.
- Omit any part that has nothing: no pulse when every bucket is zero, no phrase
  when the turn is `Unknown`, no `up` when duration is `None`, no `v` when the
  version is `None`. Join what remains with the existing
  `AGENT_ROW_SEPARATOR`. A row where all four are absent draws no second line
  at all — never an empty indented gap.
- The whole line is one `Label::truncate()`, same as the identity, for the same
  reason.
- Ordering is by value under truncation: pulse, turn, duration, version.

Phrases as consts:

```rust
/// The turn phrases. Plain words rather than glyphs: the pane's font coverage
/// is a thing M14 already had to prove once, and "your turn" needs no legend.
const TURN_IN_LOOP: &str = "in the loop";
const TURN_YOUR_TURN: &str = "your turn";
```

`your turn` is the row a human is looking for. Draw it in `AMBER` — the same ink
the pane already uses for "this wants attention" — while `in the loop` takes the
dim ink of the rest of the line. That is the only colour in the second line.

### 2.3 The pulse strip

Write a small dedicated painter. Do **not** reuse the M13 sparkline: that one
draws a percentage series over 24 h, scaled 0..100 against a window, and this is
a count series over 10 min scaled against its own busiest minute. Sharing the
function would mean two callers arguing over one set of assumptions, which is
how the M13 painter would end up with a mode flag.

- `PULSE_BUCKETS` bars, 2 px wide, 1 px gap, 7 px tall at most: 29 px of row.
- Height per bar is `count / max(pulse)` of the full height, rounded up to at
  least 1 px for any non-zero bucket, so a single record in a minute is visibly
  different from silence.
- Row-relative scaling, and say why in the doc comment: the question a reader
  asks of this strip is "is it speeding up or dying", not "is this agent busier
  than that one".
- Ink: the row's own state colour at reduced alpha, so a working row's pulse is
  green and an idle row's is amber without a second palette.
- A pulse whose buckets are all zero paints nothing and allocates nothing.

### 2.4 The demo

`--agents-demo` is how the owner reviews this, so the fixture set must show the
whole feature in one look. Extend `demo_agents` to six rows:

1. Claude, working, **in the loop**, busy pulse rising, `up 12m`, a version.
2. Claude, working, **your turn**, pulse tailing off to nothing in the last two
   buckets — the shape of an agent that stopped and is waiting.
3. Claude, working, subagent (`· sub`), short duration, flat busy pulse.
4. Codex, idle, no turn phrase (that is the honest Codex row), a branch, a
   version — and this row is *why* Codex shows no phrase, so the doc comment
   says it.
5. Codex, recent, no branch, one line only.
6. Claude, **older than two hours**, so the demo opens showing `// 1 older
   today` and the toggle can be clicked.

Update the function's doc comment to describe the new set — the existing one
enumerates four rows and would become false.

### 2.5 UI tests

- The split: a fixture list spanning the two-hour line renders the recent rows
  and the toggle; flipping `show_older` renders all of them.
- `1 older today` and `2 older today` — the plural.
- The second line appears for `Working`/`Idle` and never for `Recent`.
- A row with `Unknown` turn, `None` duration, `None` version and a zero pulse
  draws exactly one line.
- `your turn` is painted in `AMBER`, in both themes, via the existing
  painted-shapes helpers.
- The UI's own sentinel test (the M15 one at the bottom of the file) must cover
  the new fields, same standard as §1.6.

### 2.6 README

The agents paragraph gains a sentence on the two-hour default and one on turn
state. Keep it to two sentences; the README is not release notes.

---

## §4a — protected files, byte patches

`SECURITY.md`, `THREAT_MODEL.md`, `invariants.manifest` and the invariant-8
tests are §4.1 protected. Apply these exactly: extract each OLD and NEW block
from this file programmatically, confirm each OLD appears exactly once before
and each NEW exactly once after, and **never retype one**. If an OLD does not
match, stop and report — do not adapt it.

### Patch A — `SECURITY.md`, invariant 8

OLD:
```
extracting a fixed allowlist of metadata keys — ids, timestamps, record types, working directory, git branch — and nothing else.
```

NEW:
```
extracting a fixed allowlist of metadata keys — ids, timestamps, record types, working directory, git branch, and the CLI's own version string — and nothing else. Key lookup searches three levels of nested objects, because the Codex CLI files a session's branch two levels down; the price of that reach is paid by a companion list of names that may never join the allowlist (`content`, `text`, `message`, `model`, `toolUseResult` and their kin), so a lookup can descend into a record without ever being able to return the words inside a message.
```

### Patch B — `invariants.manifest`, invariant 8 bindings

OLD:
```
test: crates/usage-core/src/agents.rs::scanner_opens_only_jsonl_under_the_session_roots
```

NEW:
```
test: crates/usage-core/src/agents.rs::scanner_opens_only_jsonl_under_the_session_roots
test: crates/usage-core/src/agents.rs::no_allowlisted_key_can_ever_name_message_content
test: crates/usage-core/src/agents.rs::turn_state_is_read_from_the_record_type_alone
```

### Patch C — `THREAT_MODEL.md`, the invariant-8 row

OLD:
```
| 8. Agent visibility is metadata-only | `usage_core::agents` — allowlisted key extraction; content payloads never deserialized | `sentinel_content_never_reaches_any_output`, `extraction_is_welded_to_the_allowlist_const`, `unparseable_file_still_reports_liveness_from_mtime`, `scanner_opens_only_jsonl_under_the_session_roots` |
```

NEW:
```
| 8. Agent visibility is metadata-only | `usage_core::agents` — allowlisted key extraction fenced by a forbidden-key list; content payloads never deserialized | `sentinel_content_never_reaches_any_output`, `extraction_is_welded_to_the_allowlist_const`, `unparseable_file_still_reports_liveness_from_mtime`, `scanner_opens_only_jsonl_under_the_session_roots`, `no_allowlisted_key_can_ever_name_message_content`, `turn_state_is_read_from_the_record_type_alone` |
```

### Patch D — `THREAT_MODEL.md`, T-I6

OLD:
```
*Mitigation:* invariant 8 — allowlisted metadata keys only; the sentinel-content test proves the content payload cannot reach any output type; nothing is written to disk or sent anywhere.
```

NEW:
```
*Mitigation:* invariant 8 — allowlisted metadata keys only, with a forbidden-key list that a nested lookup cannot be widened past; the sentinel-content test proves the content payload cannot reach any output type; turn state is read from a record's type and never its payload; nothing is written to disk or sent anywhere.
```

### Patch E — `crates/usage-core/src/agents.rs`, the pinned allowlist test

OLD:
```
        let mut keys = ALLOWLISTED_KEYS.to_vec();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "cwd",
                "gitBranch",
                "git_branch",
                "id",
                "isSidechain",
                "sessionId",
                "timestamp",
                "type",
            ]
        );

        let line = serde_json::json!({
            "sessionId": "abcd1234",
            "timestamp": "2026-08-06T12:00:00.000Z",
            "type": "user",
            "cwd": "/home/j/dev/QuotaPane",
            "gitBranch": "main",
            "isSidechain": false,
            "message": {"role": "user", "content": SENTINEL},
            "summary": SENTINEL,
            "toolUseResult": {"stdout": SENTINEL},
            "payload": {"id": "efgh5678", "git_branch": "topic", "instructions": SENTINEL},
        });

        // Every allowlisted key is readable...
        for key in ALLOWLISTED_KEYS {
            assert!(
                read_allowlisted(&line, key).is_some(),
                "{key} should be readable from this line"
            );
        }
        // ...and every key that is not on the list is unreachable, however
        // plainly it sits in the JSON.
        for key in [
            "message",
            "summary",
            "toolUseResult",
            "content",
            "text",
            "stdout",
        ] {
```

NEW:
```
        let mut keys = ALLOWLISTED_KEYS.to_vec();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "branch",
                "cli_version",
                "cwd",
                "gitBranch",
                "git_branch",
                "id",
                "isSidechain",
                "sessionId",
                "timestamp",
                "type",
                "version",
            ]
        );

        let line = serde_json::json!({
            "sessionId": "abcd1234",
            "timestamp": "2026-08-06T12:00:00.000Z",
            "type": "user",
            "cwd": "/home/j/dev/QuotaPane",
            "gitBranch": "main",
            "isSidechain": false,
            "version": "2.0.14",
            "message": {"role": "user", "content": SENTINEL, "model": SENTINEL},
            "summary": SENTINEL,
            "toolUseResult": {"stdout": SENTINEL},
            "payload": {
                "id": "efgh5678",
                "git_branch": "topic",
                "cli_version": "0.5.1",
                "git": {"branch": "topic", "commit_message": SENTINEL},
                "instructions": SENTINEL,
            },
        });

        // Every allowlisted key is readable, including the branch two levels
        // down that MAX_KEY_DEPTH exists for...
        for key in ALLOWLISTED_KEYS {
            assert!(
                read_allowlisted(&line, key).is_some(),
                "{key} should be readable from this line"
            );
        }
        // ...and every key that is not on the list is unreachable, however
        // plainly it sits in the JSON — at any of the three depths searched.
        for key in [
            "message",
            "summary",
            "toolUseResult",
            "content",
            "text",
            "stdout",
            "model",
            "instructions",
            "commit_message",
        ] {
```

### Patch F — `crates/usage-core/src/agents.rs`, two new invariant-8 tests

OLD:
```
    // INV:8 — registered in invariants.manifest (checked in CI)
    #[test]
    fn unparseable_file_still_reports_liveness_from_mtime() {
```

NEW:
```
    // INV:8 — registered in invariants.manifest (checked in CI)
    #[test]
    fn no_allowlisted_key_can_ever_name_message_content() {
        // Depth is why this test exists. `read_allowlisted` searches three
        // levels of objects so a Codex branch at `payload.git.branch` is
        // reachable; the price of that reach is that a key added to the
        // allowlist in good faith could, from now on, be found *inside* a
        // message object rather than beside one. The guarantee that this module
        // never returns a sentence stopped being a property of the search and
        // became a property of the list — so the two lists are welded apart
        // here, and CI is what tells a reviewer they crossed them.
        for forbidden in FORBIDDEN_KEYS {
            assert!(
                !ALLOWLISTED_KEYS.contains(forbidden),
                "{forbidden} may never be allowlisted: it names content, not metadata"
            );
        }
        // A fence is only as good as its coverage: every name either CLI files
        // a payload of words under is on it.
        for name in [
            "content",
            "text",
            "message",
            "model",
            "toolUseResult",
            "commit_message",
        ] {
            assert!(
                FORBIDDEN_KEYS.contains(&name),
                "{name} must be on the forbidden list"
            );
        }
    }

    // INV:8 — registered in invariants.manifest (checked in CI)
    #[test]
    fn turn_state_is_read_from_the_record_type_alone() {
        use AgentState::{Idle, Recent, Working};
        use TurnState::{InLoop, Unknown, YourTurn};

        // Claude Code alternates two record types, and that alternation is the
        // whole signal: input to the agent means it has work in hand, output
        // from it means it has stopped and is waiting to be read.
        let claude = ProviderId::ClaudeSubscription;
        assert_eq!(turn_for(claude, Some("user"), Working), InLoop);
        assert_eq!(turn_for(claude, Some("assistant"), Working), YourTurn);
        assert_eq!(turn_for(claude, Some("assistant"), Idle), YourTurn);
        // A record type nobody here has seen is not a guess.
        assert_eq!(turn_for(claude, Some("system"), Working), Unknown);
        assert_eq!(turn_for(claude, None, Working), Unknown);
        // Codex's vocabulary does not draw the line where this module can read
        // it, and a blank beats an invented claim.
        assert_eq!(
            turn_for(ProviderId::CodexSubscription, Some("response_item"), Working),
            Unknown
        );
        // A session that ended hours ago has no turn to be in.
        assert_eq!(turn_for(claude, Some("user"), Recent), Unknown);

        // And the judgement survives a record that is otherwise nothing but
        // content: the type is read, the payload is not.
        let line = serde_json::json!({
            "timestamp": "2026-08-06T12:00:00.000Z",
            "type": "assistant",
            "message": {"role": "assistant", "content": SENTINEL},
            "summary": SENTINEL,
        });
        assert_eq!(turn_for(claude, read_str(&line, "type"), Working), YourTurn);
    }

    // INV:8 — registered in invariants.manifest (checked in CI)
    #[test]
    fn unparseable_file_still_reports_liveness_from_mtime() {
```

Both new tests need `// INV:8` coverage to satisfy `tools/check-invariants.py`;
the tags are inside the patches. Run the checker after applying and before
committing anything.

---

## Commits

Two, in order, each green on the full §3 bar before the next begins:

1. `M16: whose move is it — turn state, pulse, and a fence for the allowlist`
2. `M16: the pane opens on what is alive`

Same-change rule: Phase 1's protected-file patches belong in Phase 1's commit,
because they are the claim that Phase 1's behaviour makes.

## Mutation pass

Before writing the report, mutate and confirm each is caught. At minimum:
`MAX_KEY_DEPTH` to 2 and to 4; a `FORBIDDEN_KEYS` entry deleted; `turn_for`'s
`user`/`assistant` arms swapped; the `Recent` guard in `turn_for` removed;
`epoch_secs` ignoring the offset sign; a pulse bucket off by one; the
two-hour split flipped to `>=` on the wrong side; the second line drawn for
`Recent` rows. Quote the table in the report — survivors are findings, not
footnotes.

## CI

Wait in the **foreground** — `gh run watch <id> --exit-status`. Never a
background watcher; this session ends and takes it with it.

**GitHub Actions was in a major outage today** (incident open 2026-08-06
15:22 UTC; runs accepted but queued with no jobs ever created). If your run sits
queued with zero jobs for more than fifteen minutes, or fails before any step
runs, that is §4.6 and it is not yours: **stop, write the report with the local
§3 bar green and the CI state described exactly as you found it, and exit.** Do
not re-run more than twice. Do not change a byte of the tree to chase it. An
honest "CI could not start, here is the run id" is a complete report.

## Report

`reports/m16-endgate.md`, committed as a third commit. The house shape: what
each phase did, every deviation with its reasoning, the mutation table, the CI
state with run ids, and anything you were unsure of. Deviations are expected and
welcome — a spec written without the code in front of it gets things wrong, and
your judgement on the spot beats my guess from here. Flag them; do not silently
comply.
