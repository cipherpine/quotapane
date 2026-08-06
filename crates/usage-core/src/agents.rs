//! Local agent-session visibility: who is working right now (M15).
//!
//! Claude Code and the Codex CLI each keep an append-only JSONL log per
//! session, under a well-known directory in the user's home. This module reads
//! those logs — read-only, and only for a fixed allowlist of **metadata** keys
//! — so the window can list the sessions running on this machine. It is the
//! one module in `usage-core` that reads provider files which are not
//! credential files, and SECURITY.md invariant 8 is the promise it keeps.
//!
//! ## What is read, and what is not
//!
//! [`ALLOWLISTED_KEYS`] is the complete set of JSON keys any code path here may
//! look at, and [`read_allowlisted`] is the only way a value is ever taken out
//! of a parsed line: it refuses, at runtime, to return anything stored under a
//! key that is not on that list. The conversation payload — `message`,
//! `content`, `text`, whatever a future CLI version calls it — is therefore not
//! merely "not rendered": there is no expression in this module that can reach
//! it. Nothing read here is written to disk, sent anywhere, or logged; the
//! output type ([`AgentSession`]) has no field a sentence could occupy.
//!
//! ## Liveness is inferred, because neither format marks an end
//!
//! No line says "this session is over". What a session *does* say is that its
//! file was written to a moment ago, so the state comes from the file's
//! modification time against [`ACTIVE_WITHIN`] / [`IDLE_WITHIN`] / [`LOOKBACK`]
//! — and from nothing else. That is deliberate: it means a file this module
//! cannot parse at all still reports honestly, and a CLI version that renames
//! every key degrades to a row with a state and no name rather than to nothing.
//!
//! ## Bounded reads
//!
//! A long session's log is megabytes of conversation. Every candidate file is
//! `stat`ed first and opened only if its mtime falls inside [`LOOKBACK`]; an
//! opened file gives up at most two [`TAIL_CAP`]-sized reads — one at the start
//! (for the first line) and one at the end — and never the middle. The metadata
//! this module wants is in the first line; the last line is a second chance at
//! the same keys for a file whose first line is truncated or from a format it
//! does not recognise.
//!
//! ## Clock in, no clock read
//!
//! [`scan`] takes `now` as a parameter, like [`crate::pace`] and
//! [`crate::history`] do, so every threshold in here is testable at any moment
//! rather than only at the moment a test happens to run.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::model::ProviderId;

/// A session whose log was written within this long is **working**: mid-turn.
///
/// Two minutes. A session actually doing something appends far more often than
/// this — every tool call, every streamed message — so the threshold is not a
/// guess about typing speed but a generous ceiling on the gap between writes
/// inside one turn.
pub const ACTIVE_WITHIN: Duration = Duration::from_secs(120);

/// Written within this long, but not within [`ACTIVE_WITHIN`]: **idle**.
///
/// Half an hour. The session is open and the user may well come back to it —
/// this is the state a terminal sits in while its human reads the diff.
pub const IDLE_WITHIN: Duration = Duration::from_secs(1800);

/// Older than this and the file is not even opened: it is not a session
/// anybody is in.
///
/// Twenty-four hours, which is also the window the pane's empty state names.
/// The bound exists as much for cost as for meaning: a months-old projects
/// directory can hold thousands of logs, and this is what keeps a scan to the
/// handful of files that could possibly be interesting.
pub const LOOKBACK: Duration = Duration::from_secs(24 * 60 * 60);

/// The most bytes read from either end of one log file (16 KiB).
///
/// Two reads of at most this size, never the middle — see the module docs.
pub const TAIL_CAP: usize = 16 * 1024;

// The three thresholds are a ladder, and every state depends on their order:
// were `IDLE_WITHIN` below `ACTIVE_WITHIN` the idle band would be empty and a
// session would jump from working to recent. A compile-time fact, checked at
// compile time (the same idiom the window uses for its freshness thresholds).
const _: () = assert!(ACTIVE_WITHIN.as_secs() < IDLE_WITHIN.as_secs());
const _: () = assert!(IDLE_WITHIN.as_secs() < LOOKBACK.as_secs());

/// How many levels of objects [`read_allowlisted`] will look through: the
/// record, its children, and their children.
///
/// Three, because the Codex CLI writes its branch at `payload.git.branch` while
/// writing its id at `payload.id`, and a search that reaches one but not the
/// other produces a row that is silently missing a field. Not four: depth is
/// reach, reach is risk, and nothing either CLI writes needs more.
pub const MAX_KEY_DEPTH: usize = 3;

/// **The complete set of JSON keys this module may read.** SECURITY.md
/// invariant 8 is exactly this list plus [`read_allowlisted`].
///
/// Claude Code writes `sessionId`, `timestamp`, `type`, `cwd`, `gitBranch`,
/// `isSidechain` and `version` on its record lines; the Codex CLI's
/// `session_meta` record carries `id`, `cwd`, `cli_version` and the session's
/// branch — spelled `git_branch` beside the payload's other fields by the
/// builds on the owner's disk, and `branch` inside a nested `git` object by
/// recent ones — all wrapped in a record carrying its own `timestamp` and
/// `type`. Every one of them is an identifier, an instant, a record type, a
/// directory, a branch name, or the CLI's own version string. None of them is
/// content, and no key outside this list is readable from here at all:
/// [`read_allowlisted`] checks membership before it will hand back a value, so
/// deleting an entry below removes a capability rather than merely a mention.
///
/// The search that finds these keys is [`MAX_KEY_DEPTH`] objects deep, and
/// [`FORBIDDEN_KEYS`] is the other half of that bargain: what may never be
/// added here, however metadata-shaped it looks.
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

/// **Names that may never join [`ALLOWLISTED_KEYS`].**
///
/// The allowlist says what may be read. This says what may never be *added* —
/// the keys under which both CLIs file the actual words. It exists because
/// [`MAX_KEY_DEPTH`] is 3: the lookup can now reach inside a `message` object,
/// so the guarantee that it never returns a sentence stopped being a property
/// of the search and became a property of the list. A test welds the two
/// together, and a reviewer adding a key to one of these lists will be told by
/// CI if it is already on the other.
///
/// `model` is the entry worth explaining, because it is the one a future
/// reader will want to "fix". A model slug is not content, and if it lived
/// beside a record this module would read it — but Claude Code writes it at
/// `message.model`, inside the one object this module exists to leave shut, and
/// a depth-3 search would now find it there. Provenance comes from the CLI's
/// own version string instead (see [`AgentSession::cli_version`]).
pub const FORBIDDEN_KEYS: &[&str] = &[
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

/// How alive a session is, inferred from when its log was last written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    /// Written within [`ACTIVE_WITHIN`] — mid-turn.
    Working,
    /// Written within [`IDLE_WITHIN`] — open, waiting.
    Idle,
    /// Written within [`LOOKBACK`] — today, but over.
    Recent,
}

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

/// How many one-minute buckets a row's pulse carries.
pub const PULSE_BUCKETS: usize = 10;
/// How long one pulse bucket covers.
pub const PULSE_BUCKET: Duration = Duration::from_secs(60);
/// The most any one bucket will count to. A busier minute than this is
/// indistinguishable from this one at 7 px tall.
pub const PULSE_CAP: u32 = 999;

/// The most characters of a CLI version string kept, whatever the log says.
///
/// A version is `2.0.14`-shaped, and a field of unbounded length has no
/// business in a 320 px row even truncated.
const CLI_VERSION_CAP: usize = 16;

/// One local agent session, identified **without reading a word of it**.
///
/// Every field is either an identifier, a path component, a branch name, a
/// time, a version string, or a count of records. There is deliberately no
/// field of a type that could hold a sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSession {
    /// Which CLI the session belongs to.
    pub provider: ProviderId,
    /// The first eight characters of the session id — enough to tell two
    /// sessions in the same project apart, short enough for a 320px row.
    pub short_id: String,
    /// The working directory's basename, or a name derived from the log's own
    /// directory when the log did not say.
    pub project: String,
    /// The git branch the session was started on, when the log named one.
    pub branch: Option<String>,
    /// Working / idle / recent, from [`last_write`](Self::last_write).
    pub state: AgentState,
    /// The log file's modification time.
    pub last_write: SystemTime,
    /// How long before `now` that write was.
    pub age: Duration,
    /// True when the log marks this session as a subagent (Claude Code's
    /// `isSidechain`). Absence of the key is not a claim either way, and reads
    /// as false.
    pub is_subagent: bool,
    /// Whose move it is, from the **tail** record's type alone — see
    /// [`turn_for`]. [`TurnState::Unknown`] for anything this module cannot
    /// read without opening a payload, and for a session that is over.
    pub turn: TurnState,
    /// How long this session has been going: the **head** record's timestamp to
    /// [`last_write`](Self::last_write). `None` when the head line named no
    /// usable instant, and `None` rather than zero when the two disagree about
    /// which came first — a saturated zero would read as a claim.
    pub duration: Option<Duration>,
    /// The version string the CLI stamps on its own records, capped at 16
    /// characters. The row's provenance: which tool, roughly which build.
    pub cli_version: Option<String>,
    /// Records per minute over the ten minutes ending at `now`, oldest bucket
    /// first and newest last ([`PULSE_BUCKETS`] of them). Counts, never
    /// contents. All zeros for an [`AgentState::Recent`] session, whose rhythm
    /// nobody is watching.
    pub pulse: [u32; PULSE_BUCKETS],
}

/// Where the scan looks: the two CLI home directories, either of which may be
/// absent on a machine that has only one of the tools.
///
/// Dependency-injected exactly as the credential paths are, so tests point at
/// temporary directories and never at a real home (`DECISIONS.md` §4.4).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionRoots {
    /// Claude Code's home — sessions live under `<root>/projects/<dir>/*.jsonl`.
    pub claude: Option<PathBuf>,
    /// Codex's home — sessions live under
    /// `<root>/sessions/<YYYY>/<MM>/<DD>/rollout-*.jsonl`.
    pub codex: Option<PathBuf>,
}

impl SessionRoots {
    /// The production wiring: `~/.claude` and `$CODEX_HOME`-or-`~/.codex`.
    ///
    /// Resolution only — nothing here opens, reads, or stats a file.
    pub fn from_env() -> SessionRoots {
        roots_from(home_dir().as_deref(), std::env::var_os("CODEX_HOME"))
    }
}

/// [`SessionRoots::from_env`] with its two environment reads lifted out, so the
/// resolution rules are testable without touching a real home directory.
fn roots_from(home: Option<&Path>, codex_home: Option<std::ffi::OsString>) -> SessionRoots {
    let codex = match codex_home {
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
        _ => home.map(|h| h.join(".codex")),
    };
    SessionRoots {
        claude: home.map(|h| h.join(".claude")),
        codex,
    }
}

/// Best-effort home directory resolution without extra dependencies.
///
/// A deliberate twin of the private one in [`crate::credentials`]: that module
/// is a protected path whose whole value is being small and unchanging, and
/// four lines of duplication are cheaper than a reason to edit it.
fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

/// The state an age maps to, or `None` when the file is beyond [`LOOKBACK`] and
/// is not a session at all.
///
/// Every boundary is inclusive of the constant it names: a file written exactly
/// [`ACTIVE_WITHIN`] ago is still working.
pub fn state_for_age(age: Duration) -> Option<AgentState> {
    if age <= ACTIVE_WITHIN {
        Some(AgentState::Working)
    } else if age <= IDLE_WITHIN {
        Some(AgentState::Idle)
    } else if age <= LOOKBACK {
        Some(AgentState::Recent)
    } else {
        None
    }
}

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
pub fn turn_for(provider: ProviderId, record_type: Option<&str>, state: AgentState) -> TurnState {
    if state == AgentState::Recent {
        return TurnState::Unknown;
    }
    match provider {
        // Matched rather than compared, so a third provider has to decide.
        ProviderId::CodexSubscription => return TurnState::Unknown,
        ProviderId::ClaudeSubscription => {}
    }
    match record_type {
        Some("user") => TurnState::InLoop,
        Some("assistant") => TurnState::YourTurn,
        // A record type nobody here has seen says nothing about turns.
        _ => TurnState::Unknown,
    }
}

/// A log file worth opening: found by enumeration, kept by its mtime.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    provider: ProviderId,
    path: PathBuf,
    last_write: SystemTime,
    age: Duration,
    state: AgentState,
}

/// Every session log under `roots` whose mtime falls inside [`LOOKBACK`].
///
/// This is the whole of the module's directory traversal, and it is the reason
/// "only files inside the lookback are ever opened" is a structural fact rather
/// than a promise: [`scan`] opens what this returns and nothing else, and this
/// function opens nothing at all — `read_dir` and `metadata` only.
fn candidates(roots: &SessionRoots, now: SystemTime) -> Vec<Candidate> {
    let mut out = Vec::new();
    if let Some(root) = &roots.claude {
        for dir in child_dirs(&root.join("projects")) {
            for path in child_files(&dir) {
                if has_extension(&path, "jsonl") {
                    push_candidate(&mut out, ProviderId::ClaudeSubscription, path, now);
                }
            }
        }
    }
    if let Some(root) = &roots.codex {
        for year in child_dirs(&root.join("sessions")) {
            for month in child_dirs(&year) {
                for day in child_dirs(&month) {
                    for path in child_files(&day) {
                        if has_extension(&path, "jsonl") && has_prefix(&path, "rollout-") {
                            push_candidate(&mut out, ProviderId::CodexSubscription, path, now);
                        }
                    }
                }
            }
        }
    }
    out
}

/// Stat one enumerated file and keep it if it is recent enough to matter.
fn push_candidate(out: &mut Vec<Candidate>, provider: ProviderId, path: PathBuf, now: SystemTime) {
    let Ok(metadata) = std::fs::metadata(&path) else {
        return;
    };
    let Ok(last_write) = metadata.modified() else {
        return;
    };
    // A file stamped in the future is not an error worth dropping a row for —
    // clocks move — and reads as "just written", which is what it claims.
    let age = now.duration_since(last_write).unwrap_or(Duration::ZERO);
    let Some(state) = state_for_age(age) else {
        return;
    };
    out.push(Candidate {
        provider,
        path,
        last_write,
        age,
        state,
    });
}

/// The immediate subdirectories of `dir`, or nothing if it cannot be listed.
fn child_dirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|entry| entry.path())
        .collect()
}

/// The immediate files of `dir`, or nothing if it cannot be listed.
fn child_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|entry| entry.path())
        .collect()
}

/// Case-insensitive extension match — Windows filesystems are not case-exact.
fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .map(|e| e.eq_ignore_ascii_case(extension))
        .unwrap_or(false)
}

/// Whether the file's name starts with `prefix`.
fn has_prefix(path: &Path, prefix: &str) -> bool {
    path.file_name()
        .map(|n| n.to_string_lossy().starts_with(prefix))
        .unwrap_or(false)
}

/// **The only way a value leaves a parsed line.** Returns `None` for any key
/// not on [`ALLOWLISTED_KEYS`], however plainly that key sits in the JSON.
///
/// The search is bounded to [`MAX_KEY_DEPTH`] levels — the object itself, any
/// object it holds directly, and one more — because the Codex CLI wraps its
/// `session_meta` fields in a `payload` object and files the branch a further
/// level down inside a `git` one, and naming either wrapper here would be one
/// more key this module knows how to open. Only scalars are returned: an
/// allowlisted key whose value is an object or an array is treated as absent,
/// so no container can be dragged out whole and formatted somewhere else.
///
/// Depth is reach, and reach is what [`FORBIDDEN_KEYS`] fences: three levels is
/// enough to find `model` inside a `message`, so the promise that nothing here
/// returns a sentence is kept by the list rather than by the search.
fn read_allowlisted<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    if !ALLOWLISTED_KEYS.contains(&key) {
        return None;
    }
    search_levels(value, key, MAX_KEY_DEPTH)
}

/// [`read_allowlisted`]'s walk, once membership has been checked — and the only
/// caller is that check, so no path reaches this with an unlisted key.
///
/// `levels` counts the object levels still allowed to answer: the call with
/// `levels == 1` may look at its own keys and may not descend.
fn search_levels<'a>(
    value: &'a serde_json::Value,
    key: &str,
    levels: usize,
) -> Option<&'a serde_json::Value> {
    if levels == 0 {
        return None;
    }
    let object = value.as_object()?;
    let scalar = |v: &'a serde_json::Value| (!v.is_object() && !v.is_array()).then_some(v);
    if let Some(found) = object.get(key).and_then(scalar) {
        return Some(found);
    }
    object
        .values()
        .find_map(|nested| search_levels(nested, key, levels - 1))
}

/// An allowlisted key's value as a non-empty string.
fn read_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    let text = read_allowlisted(value, key)?.as_str()?;
    (!text.is_empty()).then_some(text)
}

/// The epoch seconds in an RFC 3339 stamp, or `None` for anything that is not
/// exactly the shape both CLIs write.
///
/// Strict on purpose: `YYYY-MM-DDTHH:MM:SS`, an optional fractional part which
/// is read and discarded, then either `Z` or `±HH:MM`, which is subtracted.
/// Anything else — a two-digit year, a missing zone, a month of 13 — is `None`,
/// and `None` costs a row its duration and its pulse while costing it nothing
/// else. This module already treats an unreadable file as a live session with
/// a thin row; an unreadable timestamp is the same kind of nothing.
///
/// Hand-rolled because `usage-core` parses no dates today and a new dependency
/// on the trust boundary costs more than thirty lines of arithmetic does. No
/// leap seconds, no local zone, no calendar beyond the proleptic Gregorian one.
fn epoch_secs(stamp: &str) -> Option<u64> {
    let bytes = stamp.as_bytes();
    // The fixed part is 19 characters and the zone is at least one more.
    if bytes.len() < 20 {
        return None;
    }
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    let year = digits(stamp, 0..4)?;
    let month = digits(stamp, 5..7)?;
    let day = digits(stamp, 8..10)?;
    let hour = digits(stamp, 11..13)?;
    let minute = digits(stamp, 14..16)?;
    // 60 would be a leap second, and this calendar has none.
    let second = digits(stamp, 17..19)?;
    if !(1..=12).contains(&month) || day < 1 || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let month_length = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if leap => 29,
        _ => 28,
    };
    if day > month_length {
        return None;
    }

    let mut zone = &stamp[19..];
    // An optional fractional part: read, checked for being digits at all, and
    // thrown away. Sub-second resolution is noise at a one-minute bucket.
    if let Some(fraction) = zone.strip_prefix('.') {
        let taken = fraction.bytes().take_while(u8::is_ascii_digit).count();
        if taken == 0 {
            return None;
        }
        zone = &fraction[taken..];
    }
    let offset = match zone.as_bytes().first().copied()? {
        b'Z' if zone.len() == 1 => 0,
        sign @ (b'+' | b'-') if zone.len() == 6 && zone.as_bytes()[3] == b':' => {
            let hours = digits(zone, 1..3)?;
            let minutes = digits(zone, 4..6)?;
            if hours > 23 || minutes > 59 {
                return None;
            }
            let magnitude = hours * 3_600 + minutes * 60;
            // "+05:30" means the stamp is ahead of UTC, so UTC is behind it.
            if sign == b'+' {
                magnitude
            } else {
                -magnitude
            }
        }
        _ => return None,
    };

    let seconds =
        days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second - offset;
    // Anything before 1970 is not a session anybody is in.
    u64::try_from(seconds).ok()
}

/// An all-digit slice of `text` as a number, or `None` if it is anything else.
fn digits(text: &str, range: std::ops::Range<usize>) -> Option<i64> {
    let slice = text.get(range)?;
    if slice.is_empty() || !slice.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    slice.parse().ok()
}

/// Days since 1970-01-01 for a proleptic-Gregorian date — the standard
/// branch-free days-from-civil conversion.
///
/// Valid for any date this module can be handed; the shifted-year trick moves
/// the leap day to the end of the internal year so no leap-day special case is
/// needed at all.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Whether a parsed line looks like a record this module understands: a
/// wrapper carrying both a `timestamp` and a `type`.
///
/// The gate on believing anything else a line says. A JSON object that happens
/// to be the tail of a truncated write, or a line from a format nobody here has
/// seen, fails it and contributes nothing.
fn is_record(value: &serde_json::Value) -> bool {
    read_allowlisted(value, "timestamp").is_some() && read_allowlisted(value, "type").is_some()
}

/// The metadata one file yields: everything [`AgentSession`] cannot get from a
/// `stat`.
#[derive(Debug, Default, PartialEq, Eq)]
struct Extracted {
    id: Option<String>,
    project: Option<String>,
    branch: Option<String>,
    is_subagent: bool,
    cli_version: Option<String>,
}

impl Extracted {
    /// Fill any field this line can fill and no field it cannot — so the first
    /// line's answer wins and the last line is the second chance.
    fn absorb(&mut self, provider: ProviderId, line: &str) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            return;
        };
        if !is_record(&value) {
            return;
        }
        let (id_key, branch_keys, version_key): (_, &[&str], _) = match provider {
            ProviderId::ClaudeSubscription => ("sessionId", &["gitBranch"][..], "version"),
            // The Codex ids and paths live in a `session_meta` record; an event
            // record carries none of them, and its `id`-shaped keys, if a later
            // version grows any, are not this session's identity.
            ProviderId::CodexSubscription => {
                if read_str(&value, "type") != Some("session_meta") {
                    return;
                }
                // Flat first, nested second: the logs already on the owner's
                // disk write `payload.git_branch` and recent builds write
                // `payload.git.branch`. Both are the same fact, so a build that
                // writes either produces the same row.
                ("id", &["git_branch", "branch"][..], "cli_version")
            }
        };
        if self.id.is_none() {
            self.id = read_str(&value, id_key).map(short_id);
        }
        if self.project.is_none() {
            self.project = read_str(&value, "cwd").and_then(basename);
        }
        if self.branch.is_none() {
            self.branch = branch_keys
                .iter()
                .find_map(|key| read_str(&value, key))
                .map(str::to_string);
        }
        if self.cli_version.is_none() {
            self.cli_version = read_str(&value, version_key).map(cap_version);
        }
        // Claude Code marks a subagent's transcript with `isSidechain: true`.
        // Best-effort by construction: absence is not a claim, so it can only
        // ever turn the flag on.
        self.is_subagent |=
            read_allowlisted(&value, "isSidechain").and_then(|v| v.as_bool()) == Some(true);
    }
}

/// The first eight characters of an id. Characters, not bytes: a non-ASCII id
/// would be a surprise, and slicing one would be a panic.
fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

/// A version string cut to [`CLI_VERSION_CAP`] characters — characters, for
/// [`short_id`]'s reason.
fn cap_version(version: &str) -> String {
    version.chars().take(CLI_VERSION_CAP).collect()
}

/// The last path component of a working directory, for either OS's separator —
/// the log may have been written on the other one.
fn basename(cwd: &str) -> Option<String> {
    let trimmed = cwd.trim_end_matches(['/', '\\']);
    let name = trimmed.rsplit(['/', '\\']).next()?;
    (!name.is_empty()).then(|| name.to_string())
}

/// The project name for a file whose contents said nothing: the last segment of
/// the directory the log lives in.
///
/// For Claude Code that directory *is* the working directory, encoded with its
/// separators flattened to `-`, so its last segment is the project. For Codex
/// it is a day of the month, which says little — but a degraded row that is
/// honestly thin beats no row for a session that is demonstrably running.
fn fallback_project(path: &Path) -> Option<String> {
    let dir = path.parent()?.file_name()?.to_string_lossy().to_string();
    dir.rsplit('-')
        .find(|segment| !segment.is_empty())
        .map(str::to_string)
}

/// The short id for a file whose contents said nothing: from its own name,
/// which is the session id for Claude Code and `rollout-<stamp>-<id>` for
/// Codex.
fn fallback_id(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    short_id(stem.strip_prefix("rollout-").unwrap_or(stem.as_str()))
}

/// Read a candidate's first line, and **every complete line** of its last
/// [`TAIL_CAP`] bytes.
///
/// The tail arrives whole rather than as one line because the pulse counts
/// records per minute and the records are already in memory: the two bounded
/// reads are exactly the two M15 made, and the extra cost is parsing bytes that
/// had been read and thrown away. What is *believed* stays narrow — [`scan`]
/// hands only the last of these lines to [`Extracted::absorb`].
///
/// `None` for a file that cannot be opened or read — which is not an error
/// here, only the absence of a name to go with a state.
fn read_head_and_tail(path: &Path) -> Option<(String, Vec<String>)> {
    // Read-only, and stated explicitly rather than relying on `File::open`'s
    // default: this module reads other people's files and never writes one.
    let mut file = std::fs::OpenOptions::new().read(true).open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let cap = TAIL_CAP as u64;

    let mut head_bytes = vec![0u8; cap.min(len) as usize];
    file.read_exact(&mut head_bytes).ok()?;
    // Lossy on purpose: a half-written multi-byte character at a read boundary
    // is a normal thing to find in a file another process is appending to, and
    // it is not a reason to lose the line.
    let head_text = String::from_utf8_lossy(&head_bytes).to_string();
    let head = head_text.lines().next().unwrap_or_default().to_string();

    let tail = if len <= cap {
        // The whole file was the head read, so every line of it is complete.
        head_text.lines().map(str::to_string).collect()
    } else {
        file.seek(SeekFrom::Start(len - cap)).ok()?;
        let mut tail_bytes = Vec::new();
        file.read_to_end(&mut tail_bytes).ok()?;
        // The chunk starts mid-line, so its first line is a fragment of one
        // that began before the window and is dropped unread.
        String::from_utf8_lossy(&tail_bytes)
            .lines()
            .skip(1)
            .map(str::to_string)
            .collect()
    };
    Some((head, tail))
}

/// A line's parsed form, but only if it is a record this module recognises.
///
/// The gate every per-line read goes through, so "parse, check, then read one
/// allowlisted key" is one expression at each of the three call sites rather
/// than three chances to forget the check.
fn record(line: &str) -> Option<serde_json::Value> {
    let value = serde_json::from_str::<serde_json::Value>(line).ok()?;
    is_record(&value).then_some(value)
}

/// A record line's `timestamp` as epoch seconds.
fn record_stamp(line: &str) -> Option<u64> {
    epoch_secs(read_str(&record(line)?, "timestamp")?)
}

/// How many records each of the last [`PULSE_BUCKETS`] minutes holds, oldest
/// bucket first, counted from the tail lines and nothing else.
///
/// Content-free by construction: a line contributes a `+1` to one bucket or it
/// contributes nothing. A line that will not parse, is not a record, has no
/// timestamp, has an unreadable one, or is older than the strip covers is not
/// an error — it is simply not a beat.
fn pulse_from(lines: &[String], now: SystemTime) -> [u32; PULSE_BUCKETS] {
    let mut buckets = [0u32; PULSE_BUCKETS];
    let Ok(elapsed) = now.duration_since(SystemTime::UNIX_EPOCH) else {
        return buckets;
    };
    let now_secs = elapsed.as_secs();
    let bucket = PULSE_BUCKET.as_secs();
    let span = bucket * PULSE_BUCKETS as u64;
    for line in lines {
        let Some(stamp) = record_stamp(line) else {
            continue;
        };
        // A stamp in the future reads as "just now", exactly as a future mtime
        // does in `push_candidate`: clocks move, and it is not worth a row.
        let age = now_secs.saturating_sub(stamp);
        if age >= span {
            continue;
        }
        let index = PULSE_BUCKETS - 1 - (age / bucket) as usize;
        buckets[index] = buckets[index].saturating_add(1).min(PULSE_CAP);
    }
    buckets
}

/// How long a session whose head record is `head` has been going, given when
/// its log was last written.
///
/// `None` for a head line that names no usable instant, and `None` — never zero
/// — when the head stamp is *after* the last write. A clock that moved between
/// the two is not evidence of a session that lasted no time; a saturated zero
/// would say "just started", which is a claim this cannot make.
fn duration_from(head: &str, last_write: SystemTime) -> Option<Duration> {
    let started = record_stamp(head)?;
    let ended = last_write
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()?
        .as_secs();
    ended.checked_sub(started).map(Duration::from_secs)
}

/// Every local agent session alive within [`LOOKBACK`], newest write first.
///
/// The whole feature: enumerate, stat, open the few that are recent, take the
/// allowlisted keys, and return rows. Nothing is written, nothing is sent, and
/// a file that yields nothing still yields its liveness.
pub fn scan(roots: &SessionRoots, now: SystemTime) -> Vec<AgentSession> {
    let mut sessions: Vec<AgentSession> = candidates(roots, now)
        .into_iter()
        .map(|candidate| {
            let mut extracted = Extracted::default();
            let mut turn = TurnState::Unknown;
            let mut duration = None;
            let mut pulse = [0u32; PULSE_BUCKETS];
            if let Some((head, tail_lines)) = read_head_and_tail(&candidate.path) {
                extracted.absorb(candidate.provider, &head);
                // Only the last tail line is believed — widening what is read
                // must not widen what is believed.
                if let Some(tail) = tail_lines.last() {
                    if tail != &head {
                        extracted.absorb(candidate.provider, tail);
                    }
                    // ...and only the tail record may set the turn: the head
                    // line is the session's first, and what it says about whose
                    // move it is went stale hours ago.
                    let parsed = record(tail);
                    let record_type = parsed.as_ref().and_then(|value| read_str(value, "type"));
                    turn = turn_for(candidate.provider, record_type, candidate.state);
                }
                duration = duration_from(&head, candidate.last_write);
                // A finished session's rhythm is not a thing anyone is
                // watching, and skipping it keeps the per-frame cost on the
                // rows that matter.
                if candidate.state != AgentState::Recent {
                    pulse = pulse_from(&tail_lines, now);
                }
            }
            AgentSession {
                provider: candidate.provider,
                short_id: extracted.id.unwrap_or_else(|| fallback_id(&candidate.path)),
                project: extracted
                    .project
                    .or_else(|| fallback_project(&candidate.path))
                    .unwrap_or_default(),
                branch: extracted.branch,
                state: candidate.state,
                last_write: candidate.last_write,
                age: candidate.age,
                is_subagent: extracted.is_subagent,
                turn,
                duration,
                cli_version: extracted.cli_version,
                pulse,
            }
        })
        .collect();
    // Newest write first, then by id so a tie is stable rather than whatever
    // order the directory happened to enumerate in.
    sessions.sort_by(|a, b| {
        b.last_write
            .cmp(&a.last_write)
            .then_with(|| a.short_id.cmp(&b.short_id))
    });
    sessions
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Planted in every fixture's content fields. If this string ever reaches
    /// an output, the module read something it must never read.
    const SENTINEL: &str = "SENTINEL-DO-NOT-SURFACE";

    /// A private directory per test — the tests in this module run in parallel
    /// in one process, so the test's own name is part of the path.
    fn temp_root(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("quotapane-agents-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn write_bytes(path: &Path, contents: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    /// Backdate a file's mtime so one scan can see several states at once.
    fn age_file(path: &Path, age: Duration) {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .expect("fixture must exist");
        file.set_modified(SystemTime::now() - age).unwrap();
    }

    /// A Claude Code transcript: a user line and an assistant line, both
    /// carrying the metadata keys, both carrying content that must not surface.
    fn claude_log(session_id: &str, cwd: &str, branch: Option<&str>, sidechain: bool) -> String {
        let head = serde_json::json!({
            "parentUuid": serde_json::Value::Null,
            "isSidechain": sidechain,
            "userType": "external",
            "cwd": cwd,
            "sessionId": session_id,
            "version": "2.1.4",
            "gitBranch": branch,
            "type": "user",
            "message": {"role": "user", "content": format!("{SENTINEL} — please refactor")},
            "uuid": "3f1a0f5e-0000-4000-8000-000000000001",
            "timestamp": "2026-08-06T12:00:00.000Z",
        });
        let tail = serde_json::json!({
            "parentUuid": "3f1a0f5e-0000-4000-8000-000000000001",
            "isSidechain": sidechain,
            "cwd": cwd,
            "sessionId": session_id,
            "gitBranch": branch,
            "type": "assistant",
            "message": {
                "role": "assistant",
                "content": [{"type": "text", "text": SENTINEL}],
            },
            "uuid": "3f1a0f5e-0000-4000-8000-000000000002",
            "timestamp": "2026-08-06T12:00:09.000Z",
        });
        format!("{head}\n{tail}\n")
    }

    /// A Claude transcript with one record per stamp, alternating `user` and
    /// `assistant` as a real one does — so the last line decides the turn.
    fn claude_log_at(session_id: &str, cwd: &str, stamps: &[&str]) -> String {
        let mut out = String::new();
        for (index, stamp) in stamps.iter().enumerate() {
            let line = serde_json::json!({
                "isSidechain": false,
                "cwd": cwd,
                "sessionId": session_id,
                "version": "2.0.14",
                "gitBranch": "main",
                "type": if index % 2 == 0 { "user" } else { "assistant" },
                "message": {"role": "user", "content": SENTINEL, "model": SENTINEL},
                "timestamp": stamp,
            });
            out.push_str(&format!("{line}\n"));
        }
        out
    }

    /// A Codex rollout: the `session_meta` first line and an event line.
    fn codex_log(id: &str, cwd: &str, branch: Option<&str>) -> String {
        let head = serde_json::json!({
            "timestamp": "2026-08-06T12:00:00.000Z",
            "type": "session_meta",
            "payload": {
                "id": id,
                "timestamp": "2026-08-06T12:00:00.000Z",
                "cwd": cwd,
                "originator": "codex_cli_rs",
                "cli_version": "0.42.0",
                "git_branch": branch,
                "instructions": SENTINEL,
            },
        });
        let tail = serde_json::json!({
            "timestamp": "2026-08-06T12:00:11.000Z",
            "type": "event_msg",
            "payload": {"type": "agent_message", "message": SENTINEL},
        });
        format!("{head}\n{tail}\n")
    }

    /// A Codex rollout from a build that files the branch two levels down, at
    /// `payload.git.branch` — the nesting [`MAX_KEY_DEPTH`] was widened for.
    fn codex_log_nested_branch(id: &str, cwd: &str, branch: &str) -> String {
        let head = serde_json::json!({
            "timestamp": "2026-08-06T12:00:00.000Z",
            "type": "session_meta",
            "payload": {
                "id": id,
                "cwd": cwd,
                "originator": "codex_cli_rs",
                "cli_version": "0.52.0",
                "git": {
                    "branch": branch,
                    "commit_hash": "0f1e2d3c4b5a",
                    "commit_message": SENTINEL,
                },
                "instructions": SENTINEL,
            },
        });
        let tail = serde_json::json!({
            "timestamp": "2026-08-06T12:00:11.000Z",
            "type": "event_msg",
            "payload": {"type": "agent_message", "message": SENTINEL},
        });
        format!("{head}\n{tail}\n")
    }

    /// A fixed instant, from a stamp of the shape both CLIs write.
    ///
    /// The pulse and duration tests need `now` and the fixture's own records on
    /// one clock, which is what [`scan`]'s `now` parameter is for. It leans on
    /// [`epoch_secs`], which is itself pinned against hand-computed constants
    /// in `epoch_secs_reads_the_stamp_both_clis_write_and_refuses_everything_else`.
    fn at(stamp: &str) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(epoch_secs(stamp).expect("a valid test stamp"))
    }

    /// Set a fixture's mtime to an absolute instant — [`age_file`]'s twin for
    /// the tests that also fix `now`.
    fn set_mtime(path: &Path, when: SystemTime) {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .expect("fixture must exist");
        file.set_modified(when).unwrap();
    }

    /// Every string a session can show, plus its `Debug` — the whole reachable
    /// output surface of the public types (neither has a `Display` impl).
    fn every_output(sessions: &[AgentSession]) -> String {
        let mut out = format!("{sessions:?}");
        for session in sessions {
            out.push_str(&session.short_id);
            out.push_str(&session.project);
            out.push_str(session.branch.as_deref().unwrap_or_default());
            out.push_str(session.cli_version.as_deref().unwrap_or_default());
            out.push_str(&format!("{:?}{:?}", session.provider, session.state));
            // Every field M16 added formats here too. A new field this helper
            // does not format is a hole in invariant 8: the sentinel test can
            // only fail on text it is shown.
            out.push_str(&format!(
                "{:?}{:?}{:?}",
                session.turn, session.duration, session.pulse
            ));
        }
        out
    }

    // ---------------------------------------------------------------- states

    #[test]
    fn the_states_are_a_ladder_and_every_boundary_comes_from_a_const() {
        assert!(ACTIVE_WITHIN < IDLE_WITHIN);
        assert!(IDLE_WITHIN < LOOKBACK);

        let step = Duration::from_secs(1);
        for (age, expected) in [
            (Duration::ZERO, Some(AgentState::Working)),
            (ACTIVE_WITHIN - step, Some(AgentState::Working)),
            (ACTIVE_WITHIN, Some(AgentState::Working)),
            (ACTIVE_WITHIN + step, Some(AgentState::Idle)),
            (IDLE_WITHIN - step, Some(AgentState::Idle)),
            (IDLE_WITHIN, Some(AgentState::Idle)),
            (IDLE_WITHIN + step, Some(AgentState::Recent)),
            (LOOKBACK - step, Some(AgentState::Recent)),
            (LOOKBACK, Some(AgentState::Recent)),
            (LOOKBACK + step, None),
        ] {
            assert_eq!(state_for_age(age), expected, "age {age:?}");
        }
    }

    #[test]
    fn the_roots_resolve_to_the_documented_directories() {
        let home = PathBuf::from("/home/tester");
        let plain = roots_from(Some(&home), None);
        assert_eq!(plain.claude, Some(home.join(".claude")));
        assert_eq!(plain.codex, Some(home.join(".codex")));

        // $CODEX_HOME wins when set and non-empty, exactly as the credential
        // loader treats it; an empty value is not a setting.
        let moved = roots_from(Some(&home), Some(std::ffi::OsString::from("/opt/codex")));
        assert_eq!(moved.codex, Some(PathBuf::from("/opt/codex")));
        assert_eq!(moved.claude, Some(home.join(".claude")));
        assert_eq!(
            roots_from(Some(&home), Some(std::ffi::OsString::new())).codex,
            Some(home.join(".codex"))
        );

        // No home is not an error; it is nowhere to look.
        assert_eq!(roots_from(None, None), SessionRoots::default());
    }

    // ------------------------------------------------------- invariant 8

    // INV:8 — registered in invariants.manifest (checked in CI)
    #[test]
    fn sentinel_content_never_reaches_any_output() {
        let root = temp_root("sentinel");
        write(
            &root.join("claude/projects/-home-j-dev-QuotaPane/aaaa1111-2222-3333.jsonl"),
            &claude_log(
                "aaaa1111-2222-3333",
                "/home/j/dev/QuotaPane",
                Some("main"),
                false,
            ),
        );
        write(
            &root.join("codex/sessions/2026/08/06/rollout-2026-08-06T12-00-00-bbbb2222.jsonl"),
            &codex_log("bbbb2222-3333-4444", "/home/j/dev/other", Some("feat/x")),
        );

        let roots = SessionRoots {
            claude: Some(root.join("claude")),
            codex: Some(root.join("codex")),
        };
        let sessions = scan(&roots, SystemTime::now());
        assert_eq!(sessions.len(), 2, "{sessions:?}");

        let output = every_output(&sessions);
        assert!(
            !output.contains(SENTINEL),
            "conversation content reached an output: {output}"
        );
        // The fixtures really do carry it, so the assertion above is not
        // passing because there was nothing to find.
        let fixture = claude_log("x", "/tmp/x", None, false);
        assert!(fixture.contains(SENTINEL), "the fixture lost its sentinel");

        let _ = std::fs::remove_dir_all(&root);
    }

    // INV:8 — registered in invariants.manifest (checked in CI)
    #[test]
    fn extraction_is_welded_to_the_allowlist_const() {
        // The list itself is the claim SECURITY.md invariant 8 makes, so it is
        // pinned here rather than left to drift.
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
            assert!(
                read_allowlisted(&line, key).is_none(),
                "{key} is not allowlisted and must not be readable"
            );
        }
        // A container under an allowlisted name is not a value either.
        assert!(read_allowlisted(&serde_json::json!({"cwd": {"a": SENTINEL}}), "cwd").is_none());
        assert!(read_allowlisted(&serde_json::json!({"cwd": [SENTINEL]}), "cwd").is_none());
    }

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
            turn_for(
                ProviderId::CodexSubscription,
                Some("response_item"),
                Working
            ),
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
        let root = temp_root("unparseable");
        let path = root.join("claude/projects/-home-j-dev-QuotaPane/deadbeef-cafe.jsonl");
        write(&path, &format!("this is not json at all — {SENTINEL}\n"));

        let roots = SessionRoots {
            claude: Some(root.join("claude")),
            codex: None,
        };
        let sessions = scan(&roots, SystemTime::now());
        assert_eq!(sessions.len(), 1, "{sessions:?}");
        let session = &sessions[0];
        assert_eq!(session.state, AgentState::Working, "mtime alone decides");
        assert_eq!(session.project, "QuotaPane", "project from the directory");
        assert_eq!(session.short_id, "deadbeef", "id from the file name");
        assert_eq!(session.branch, None);
        assert!(!every_output(&sessions).contains(SENTINEL));

        // Still true when the bytes are not even UTF-8.
        write_bytes(&path, &[0xff, 0xfe, 0x00, 0x9f, b'\n']);
        let sessions = scan(&roots, SystemTime::now());
        assert_eq!(sessions.len(), 1, "{sessions:?}");
        assert_eq!(sessions[0].state, AgentState::Working);

        let _ = std::fs::remove_dir_all(&root);
    }

    // INV:8 — registered in invariants.manifest (checked in CI)
    #[test]
    fn scanner_opens_only_jsonl_under_the_session_roots() {
        let root = temp_root("shape");
        let claude = root.join("claude");
        let codex = root.join("codex");

        let wanted_claude = claude.join("projects/-home-j-dev-QuotaPane/aaaa1111.jsonl");
        write(
            &wanted_claude,
            &claude_log("aaaa1111", "/home/j/dev/QuotaPane", None, false),
        );
        let wanted_codex =
            codex.join("sessions/2026/08/06/rollout-2026-08-06T12-00-00-bbbb2222.jsonl");
        write(
            &wanted_codex,
            &codex_log("bbbb2222", "/home/j/dev/other", None),
        );

        // Everything below is the wrong extension, the wrong name, or the
        // wrong depth, and every one of them is full of sentinel.
        for decoy in [
            claude.join("projects/-home-j-dev-QuotaPane/notes.txt"),
            claude.join("projects/-home-j-dev-QuotaPane/session.json"),
            claude.join("projects/stray.jsonl"),
            claude.join("other/-home-j/deep.jsonl"),
            claude.join("todo.jsonl"),
            codex.join("sessions/2026/08/06/notes.jsonl"),
            codex.join("sessions/2026/08/rollout-shallow.jsonl"),
            codex.join("sessions/rollout-shallower.jsonl"),
            codex.join("sessions/2026/08/06/07/rollout-too-deep.jsonl"),
        ] {
            write(&decoy, &format!("{{\"leak\":\"{SENTINEL}\"}}\n"));
        }

        let roots = SessionRoots {
            claude: Some(claude),
            codex: Some(codex),
        };
        let mut opened: Vec<PathBuf> = candidates(&roots, SystemTime::now())
            .into_iter()
            .map(|c| c.path)
            .collect();
        opened.sort();
        let mut expected = vec![wanted_claude, wanted_codex];
        expected.sort();
        assert_eq!(opened, expected, "only these two files may be opened");

        let sessions = scan(&roots, SystemTime::now());
        assert_eq!(sessions.len(), 2, "{sessions:?}");
        assert!(!every_output(&sessions).contains(SENTINEL));

        let _ = std::fs::remove_dir_all(&root);
    }

    // ------------------------------------------------------------- the scan

    #[test]
    fn a_claude_transcript_reads_as_one_row() {
        let root = temp_root("claude");
        write(
            &root.join("projects/-home-j-dev-QuotaPane/a1b2c3d4-e5f6-7890.jsonl"),
            &claude_log(
                "a1b2c3d4-e5f6-7890",
                "/home/j/dev/QuotaPane",
                Some("main"),
                false,
            ),
        );
        let roots = SessionRoots {
            claude: Some(root.clone()),
            codex: None,
        };

        let sessions = scan(&roots, SystemTime::now());
        assert_eq!(sessions.len(), 1, "{sessions:?}");
        let session = &sessions[0];
        assert_eq!(session.provider, ProviderId::ClaudeSubscription);
        assert_eq!(session.short_id, "a1b2c3d4");
        assert_eq!(session.project, "QuotaPane");
        assert_eq!(session.branch.as_deref(), Some("main"));
        assert_eq!(session.state, AgentState::Working);
        assert!(!session.is_subagent);
        assert!(session.age < ACTIVE_WITHIN);

        // A Windows working directory reduces to the same basename.
        write(
            &root.join("projects/-home-j-dev-QuotaPane/a1b2c3d4-e5f6-7890.jsonl"),
            &claude_log("a1b2c3d4-e5f6-7890", "C:\\dev\\QuotaPane\\", None, false),
        );
        let sessions = scan(&roots, SystemTime::now());
        assert_eq!(sessions[0].project, "QuotaPane");
        assert_eq!(sessions[0].branch, None, "a null branch is no branch");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_codex_rollout_reads_as_one_row() {
        let root = temp_root("codex");
        write(
            &root.join("sessions/2026/08/06/rollout-2026-08-06T12-00-00-9f8e7d6c.jsonl"),
            &codex_log("9f8e7d6c-5b4a-3928", "/home/j/dev/other", Some("feat/x")),
        );
        let roots = SessionRoots {
            claude: None,
            codex: Some(root.clone()),
        };

        let sessions = scan(&roots, SystemTime::now());
        assert_eq!(sessions.len(), 1, "{sessions:?}");
        let session = &sessions[0];
        assert_eq!(session.provider, ProviderId::CodexSubscription);
        assert_eq!(session.short_id, "9f8e7d6c");
        assert_eq!(session.project, "other");
        assert_eq!(session.branch.as_deref(), Some("feat/x"));
        assert!(!session.is_subagent, "Codex has no sidechain concept");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_mixed_tree_lists_both_providers_newest_first() {
        let root = temp_root("mixed");
        let claude = root.join("claude");
        let codex = root.join("codex");

        let working = claude.join("projects/-home-j-dev-QuotaPane/aaaa1111.jsonl");
        write(
            &working,
            &claude_log("aaaa1111", "/home/j/dev/QuotaPane", Some("main"), false),
        );
        let idle = codex.join("sessions/2026/08/06/rollout-2026-08-06T09-00-00-bbbb2222.jsonl");
        write(
            &idle,
            &codex_log("bbbb2222", "/home/j/work/api", Some("trunk")),
        );
        let recent = claude.join("projects/-home-j-dev-notes/cccc3333.jsonl");
        write(
            &recent,
            &claude_log("cccc3333", "/home/j/dev/notes", None, false),
        );

        age_file(&idle, ACTIVE_WITHIN + Duration::from_secs(60));
        age_file(&recent, IDLE_WITHIN + Duration::from_secs(60));

        let roots = SessionRoots {
            claude: Some(claude),
            codex: Some(codex),
        };
        let sessions = scan(&roots, SystemTime::now());
        assert_eq!(sessions.len(), 3, "{sessions:?}");
        assert_eq!(
            sessions
                .iter()
                .map(|s| (s.provider, s.state, s.short_id.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (
                    ProviderId::ClaudeSubscription,
                    AgentState::Working,
                    "aaaa1111"
                ),
                (ProviderId::CodexSubscription, AgentState::Idle, "bbbb2222"),
                (
                    ProviderId::ClaudeSubscription,
                    AgentState::Recent,
                    "cccc3333"
                ),
            ],
            "newest write first"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_empty_tree_and_an_absent_one_both_list_nothing() {
        let root = temp_root("empty");
        std::fs::create_dir_all(root.join("claude/projects")).unwrap();
        let roots = SessionRoots {
            claude: Some(root.join("claude")),
            codex: Some(root.join("codex-does-not-exist")),
        };
        assert!(scan(&roots, SystemTime::now()).is_empty());
        assert!(scan(&SessionRoots::default(), SystemTime::now()).is_empty());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_outside_the_lookback_is_never_opened() {
        let root = temp_root("lookback");
        let old = root.join("projects/-home-j-dev-QuotaPane/old-11112222.jsonl");
        // Not valid UTF-8, so a code path that read it before checking its age
        // would have to deal with bytes it cannot decode.
        write_bytes(&old, &[0xff, 0xfe, 0xfd, b'\n']);
        age_file(&old, LOOKBACK + Duration::from_secs(3600));

        let fresh = root.join("projects/-home-j-dev-QuotaPane/new-33334444.jsonl");
        write(
            &fresh,
            &claude_log("33334444", "/home/j/dev/QuotaPane", None, false),
        );

        let roots = SessionRoots {
            claude: Some(root.clone()),
            codex: None,
        };
        let opened: Vec<PathBuf> = candidates(&roots, SystemTime::now())
            .into_iter()
            .map(|c| c.path)
            .collect();
        assert_eq!(
            opened,
            vec![fresh],
            "the old file is never handed to a read"
        );
        assert_eq!(scan(&roots, SystemTime::now()).len(), 1);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn is_sidechain_marks_a_subagent_and_its_absence_does_not() {
        let root = temp_root("sidechain");
        let dir = root.join("projects/-home-j-dev-QuotaPane");
        write(
            &dir.join("sub-11112222.jsonl"),
            &claude_log("11112222", "/home/j/dev/QuotaPane", Some("main"), true),
        );
        write(
            &dir.join("main-33334444.jsonl"),
            &claude_log("33334444", "/home/j/dev/QuotaPane", Some("main"), false),
        );
        // A transcript from a version that never wrote the key at all.
        let no_key = serde_json::json!({
            "cwd": "/home/j/dev/QuotaPane",
            "sessionId": "55556666",
            "gitBranch": "main",
            "type": "user",
            "message": {"role": "user", "content": SENTINEL},
            "timestamp": "2026-08-06T12:00:00.000Z",
        });
        write(&dir.join("nokey-55556666.jsonl"), &format!("{no_key}\n"));

        let roots = SessionRoots {
            claude: Some(root.clone()),
            codex: None,
        };
        let sessions = scan(&roots, SystemTime::now());
        let flag = |id: &str| {
            sessions
                .iter()
                .find(|s| s.short_id == id)
                .unwrap_or_else(|| panic!("{id} missing from {sessions:?}"))
                .is_subagent
        };
        assert!(flag("11112222"), "isSidechain: true marks a subagent");
        assert!(!flag("33334444"), "isSidechain: false does not");
        assert!(!flag("55556666"), "an absent key does not");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_codex_date_tree_is_enumerated_across_a_month_boundary() {
        let root = temp_root("months");
        for (path, id) in [
            (
                "sessions/2026/07/31/rollout-2026-07-31T23-59-00-aaaa1111.jsonl",
                "aaaa1111",
            ),
            (
                "sessions/2026/08/01/rollout-2026-08-01T00-01-00-bbbb2222.jsonl",
                "bbbb2222",
            ),
            (
                "sessions/2025/12/31/rollout-2025-12-31T23-00-00-cccc3333.jsonl",
                "cccc3333",
            ),
        ] {
            write(
                &root.join(path),
                &codex_log(id, "/home/j/dev/QuotaPane", None),
            );
        }
        let roots = SessionRoots {
            claude: None,
            codex: Some(root.clone()),
        };
        let mut ids: Vec<String> = scan(&roots, SystemTime::now())
            .into_iter()
            .map(|s| s.short_id)
            .collect();
        ids.sort();
        assert_eq!(ids, ["aaaa1111", "bbbb2222", "cccc3333"]);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn only_the_two_ends_of_a_long_file_are_read() {
        let root = temp_root("bounded");
        let path = root.join("projects/-home-j-dev-QuotaPane/big-77778888.jsonl");
        let log = claude_log("77778888", "/home/j/dev/QuotaPane", Some("main"), false);
        let (head, tail) = log.split_once('\n').unwrap();
        // A megabyte of middle, every line of it content.
        let filler = format!(
            "{}\n",
            serde_json::json!({
                "type": "assistant",
                "timestamp": "2026-08-06T12:00:05.000Z",
                "message": {"role": "assistant", "content": SENTINEL},
            })
        );
        let mut body = String::from(head);
        body.push('\n');
        for _ in 0..(1024 * 1024 / filler.len()) {
            body.push_str(&filler);
        }
        body.push_str(tail);
        write(&path, &body);
        assert!(body.len() > TAIL_CAP * 4, "fixture must exceed the caps");

        let roots = SessionRoots {
            claude: Some(root.clone()),
            codex: None,
        };
        let sessions = scan(&roots, SystemTime::now());
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].short_id, "77778888");
        assert_eq!(sessions[0].branch.as_deref(), Some("main"));
        assert!(!every_output(&sessions).contains(SENTINEL));

        let _ = std::fs::remove_dir_all(&root);
    }

    // -------------------------------------------------------------- M16: depth

    #[test]
    fn the_lookup_reaches_exactly_three_levels_and_no_further() {
        assert_eq!(MAX_KEY_DEPTH, 3);

        // Level 1 is beside the record, level 2 is Codex's `payload`, and level
        // 3 is the `git` object a recent build files the branch in.
        let beside = serde_json::json!({"branch": "topic"});
        let wrapped = serde_json::json!({"payload": {"branch": "topic"}});
        let nested = serde_json::json!({"payload": {"git": {"branch": "topic"}}});
        for (depth, line) in [(1, &beside), (2, &wrapped), (3, &nested)] {
            assert_eq!(read_str(line, "branch"), Some("topic"), "depth {depth}");
        }

        // Four is out of reach, and that is the other half of the fence: every
        // level of reach is another object this module can see inside.
        let deeper = serde_json::json!({"payload": {"git": {"remote": {"branch": "topic"}}}});
        assert_eq!(
            read_str(&deeper, "branch"),
            None,
            "depth 4 is not reachable"
        );
    }

    #[test]
    fn the_forbidden_list_is_pinned_the_way_the_allowlist_is() {
        // The allowlist is pinned by the invariant-8 test above; this is the
        // same discipline for the list that fences it, so removing a name is a
        // failing test rather than a quiet loss of a guard.
        let mut keys = FORBIDDEN_KEYS.to_vec();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "command",
                "commit_message",
                "content",
                "input",
                "instructions",
                "message",
                "model",
                "output",
                "stderr",
                "stdout",
                "summary",
                "text",
                "toolUseResult",
            ]
        );
    }

    // ------------------------------------------------------------ M16: clock

    #[test]
    fn epoch_secs_reads_the_stamp_both_clis_write_and_refuses_everything_else() {
        for (stamp, expected) in [
            ("1970-01-01T00:00:00Z", 0),
            ("2026-08-06T12:00:00Z", 1_786_017_600),
            // A fractional part is read and discarded, at any length.
            ("2026-08-06T12:00:00.123Z", 1_786_017_600),
            ("2026-08-06T12:00:00.000000001Z", 1_786_017_600),
            // An offset is subtracted, with its sign: a stamp ahead of UTC
            // happened earlier than the same digits in UTC, not later.
            ("2026-08-06T12:00:00+05:30", 1_785_997_800),
            ("2026-08-06T12:00:00-08:00", 1_786_046_400),
            // A leap day, and a month boundary either side of a second.
            ("2024-02-29T00:00:00Z", 1_709_164_800),
            ("2026-07-31T23:59:59Z", 1_785_542_399),
            ("2026-08-01T00:00:00Z", 1_785_542_400),
        ] {
            assert_eq!(epoch_secs(stamp), Some(expected), "{stamp}");
        }

        for malformed in [
            "",
            "26-08-06T12:00:00Z",         // a two-digit year
            "20xx-08-06T12:00:00Z",       // digits that are not
            "2026-08-06T12:00:00",        // no zone at all
            "2026-13-06T12:00:00Z",       // a month of 13
            "2026-02-30T00:00:00Z",       // a day February does not have
            "2026-08-06 12:00:00Z",       // a space where the T goes
            "2026-08-06T25:00:00Z",       // an hour of 25
            "2026-08-06T12:00:60Z",       // a leap second, which this has none of
            "2026-08-06T12:00:00+0530",   // an offset missing its colon
            "2026-08-06T12:00:00+05:30x", // and one with something after it
            "2026-08-06T12:00:00X",       // a zone designator nobody writes
            "2026-08-06T12:00:00.Z",      // a point with no fraction after it
            "1969-12-31T23:59:59Z",       // before the epoch
        ] {
            assert_eq!(epoch_secs(malformed), None, "{malformed:?} must not parse");
        }

        // A calendar spot-check the conversion cannot fudge: every day of a
        // leap year is exactly one day after the last.
        let mut previous = epoch_secs("2024-01-01T00:00:00Z").unwrap();
        for (month, length) in [
            (1, 31),
            (2, 29),
            (3, 31),
            (4, 30),
            (5, 31),
            (6, 30),
            (7, 31),
            (8, 31),
            (9, 30),
            (10, 31),
            (11, 30),
            (12, 31),
        ] {
            for day in 1..=length {
                let stamp = format!("2024-{month:02}-{day:02}T00:00:00Z");
                let secs = epoch_secs(&stamp).unwrap_or_else(|| panic!("{stamp}"));
                if !(month == 1 && day == 1) {
                    assert_eq!(secs - previous, 86_400, "{stamp}");
                }
                previous = secs;
            }
            // ...and the day after that month's last is not in it.
            assert_eq!(
                epoch_secs(&format!("2024-{month:02}-{:02}T00:00:00Z", length + 1)),
                None,
                "month {month} has only {length} days"
            );
        }
    }

    // ------------------------------------------------- M16: branch, both ways

    #[test]
    fn a_codex_branch_reads_the_same_flat_or_nested() {
        let root = temp_root("codex-branch");
        write(
            &root.join("sessions/2026/08/06/rollout-2026-08-06T12-00-00-aaaa1111.jsonl"),
            &codex_log("aaaa1111", "/home/j/dev/QuotaPane", Some("feat/x")),
        );
        write(
            &root.join("sessions/2026/08/06/rollout-2026-08-06T12-00-00-bbbb2222.jsonl"),
            &codex_log_nested_branch("bbbb2222", "/home/j/dev/QuotaPane", "feat/x"),
        );

        let roots = SessionRoots {
            claude: None,
            codex: Some(root.clone()),
        };
        let sessions = scan(&roots, SystemTime::now());
        assert_eq!(sessions.len(), 2, "{sessions:?}");
        for session in &sessions {
            assert_eq!(
                session.branch.as_deref(),
                Some("feat/x"),
                "both spellings are the same fact: {session:?}"
            );
        }
        // The deeper fixture is the one that needed the reach, and it must not
        // have brought anything else up with it.
        assert!(!every_output(&sessions).contains(SENTINEL));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_cli_version_comes_from_whichever_key_the_provider_writes() {
        let root = temp_root("version");
        write(
            &root.join("claude/projects/-home-j-dev-QuotaPane/aaaa1111.jsonl"),
            &claude_log("aaaa1111", "/home/j/dev/QuotaPane", Some("main"), false),
        );
        write(
            &root.join("codex/sessions/2026/08/06/rollout-2026-08-06T12-00-00-bbbb2222.jsonl"),
            &codex_log("bbbb2222", "/home/j/dev/other", Some("trunk")),
        );
        let roots = SessionRoots {
            claude: Some(root.join("claude")),
            codex: Some(root.join("codex")),
        };
        let sessions = scan(&roots, SystemTime::now());
        let version = |id: &str| {
            sessions
                .iter()
                .find(|s| s.short_id == id)
                .unwrap_or_else(|| panic!("{id} missing from {sessions:?}"))
                .cli_version
                .clone()
        };
        assert_eq!(version("aaaa1111").as_deref(), Some("2.1.4"), "Claude");
        assert_eq!(version("bbbb2222").as_deref(), Some("0.42.0"), "Codex");

        // A version string of unbounded length is cut, not carried.
        let long = root.join("claude/projects/-home-j-dev-QuotaPane/cccc3333.jsonl");
        let line = serde_json::json!({
            "sessionId": "cccc3333",
            "timestamp": "2026-08-06T12:00:00.000Z",
            "type": "user",
            "cwd": "/home/j/dev/QuotaPane",
            "version": "2.0.14-nightly.20260806.the-longest-build-tag-anyone-has-shipped",
            "message": {"role": "user", "content": SENTINEL},
        });
        write(&long, &format!("{line}\n"));
        let sessions = scan(&roots, SystemTime::now());
        let capped = sessions
            .iter()
            .find(|s| s.short_id == "cccc3333")
            .expect("the third row")
            .cli_version
            .clone()
            .expect("a version");
        assert_eq!(capped.chars().count(), 16, "{capped}");
        assert_eq!(capped, "2.0.14-nightly.2");

        let _ = std::fs::remove_dir_all(&root);
    }

    // ------------------------------------------------------------- M16: turn

    #[test]
    fn the_turn_is_read_from_the_tail_line_and_never_the_head() {
        let root = temp_root("turn");
        let dir = root.join("projects/-home-j-dev-QuotaPane");
        // Two lines: `user` then `assistant`. The head says the agent had work
        // in hand; the tail says it has stopped, and the tail is the truth.
        write(
            &dir.join("stopped-aaaa1111.jsonl"),
            &claude_log_at(
                "aaaa1111",
                "/home/j/dev/QuotaPane",
                &["2026-08-06T12:00:00Z", "2026-08-06T12:00:09Z"],
            ),
        );
        // Three lines: the same head, and a tail that is input to the agent.
        write(
            &dir.join("working-bbbb2222.jsonl"),
            &claude_log_at(
                "bbbb2222",
                "/home/j/dev/QuotaPane",
                &[
                    "2026-08-06T12:00:00Z",
                    "2026-08-06T12:00:09Z",
                    "2026-08-06T12:00:18Z",
                ],
            ),
        );
        let roots = SessionRoots {
            claude: Some(root.clone()),
            codex: None,
        };
        let sessions = scan(&roots, SystemTime::now());
        let turn = |id: &str| {
            sessions
                .iter()
                .find(|s| s.short_id == id)
                .unwrap_or_else(|| panic!("{id} missing from {sessions:?}"))
                .turn
        };
        assert_eq!(turn("aaaa1111"), TurnState::YourTurn, "tail is assistant");
        assert_eq!(turn("bbbb2222"), TurnState::InLoop, "tail is user");
        assert!(!every_output(&sessions).contains(SENTINEL));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_codex_row_and_a_finished_row_claim_no_turn() {
        let root = temp_root("turn-blank");
        let codex = root.join("codex/sessions/2026/08/06/rollout-12-00-00-aaaa1111.jsonl");
        write(
            &codex,
            &codex_log("aaaa1111", "/home/j/dev/QuotaPane", Some("main")),
        );
        let over = root.join("claude/projects/-home-j-dev-QuotaPane/bbbb2222.jsonl");
        write(
            &over,
            &claude_log_at(
                "bbbb2222",
                "/home/j/dev/QuotaPane",
                &["2026-08-06T12:00:00Z", "2026-08-06T12:00:09Z"],
            ),
        );
        age_file(&over, IDLE_WITHIN + Duration::from_secs(60));

        let roots = SessionRoots {
            claude: Some(root.join("claude")),
            codex: Some(root.join("codex")),
        };
        let sessions = scan(&roots, SystemTime::now());
        assert_eq!(sessions.len(), 2, "{sessions:?}");
        for session in &sessions {
            assert_eq!(
                session.turn,
                TurnState::Unknown,
                "neither a Codex row nor a finished one has a turn: {session:?}"
            );
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    // ------------------------------------------------------------ M16: pulse

    #[test]
    fn the_pulse_counts_records_by_the_minute_over_the_last_ten() {
        let root = temp_root("pulse");
        let now = at("2026-08-06T12:05:00Z");
        let path = root.join("projects/-home-j-dev-QuotaPane/aaaa1111.jsonl");
        write(
            &path,
            &claude_log_at(
                "aaaa1111",
                "/home/j/dev/QuotaPane",
                &[
                    "2026-08-06T11:54:00Z", // 11 min back: older than the strip
                    "2026-08-06T11:55:50Z", // 9m10s back: the oldest bucket
                    "2026-08-06T12:03:30Z", // 1m30s back
                    "2026-08-06T12:04:30Z", // 30s back
                    "2026-08-06T12:04:59Z", // 1s back
                ],
            ),
        );
        set_mtime(&path, at("2026-08-06T12:04:59Z"));

        let roots = SessionRoots {
            claude: Some(root.clone()),
            codex: None,
        };
        let sessions = scan(&roots, now);
        assert_eq!(sessions.len(), 1, "{sessions:?}");
        assert_eq!(
            sessions[0].pulse,
            [1, 0, 0, 0, 0, 0, 0, 0, 1, 2],
            "oldest bucket first, newest last"
        );
        // The line before the strip's ten minutes is not in it — the whole
        // count is 5 lines, and only 4 of them are beats.
        assert_eq!(sessions[0].pulse.iter().sum::<u32>(), 4);
        assert!(!every_output(&sessions).contains(SENTINEL));

        // A stamp from a clock that is ahead reads as "just now" rather than
        // costing the beat, exactly as a future mtime does in `push_candidate`.
        let ahead = vec![claude_log_at("x", "/tmp/x", &["2026-08-06T12:06:00Z"])
            .trim_end()
            .to_string()];
        assert_eq!(pulse_from(&ahead, now)[PULSE_BUCKETS - 1], 1);

        // And a minute busier than the cap is still the cap.
        let flood: Vec<String> = std::iter::repeat_n(
            claude_log_at("x", "/tmp/x", &["2026-08-06T12:04:30Z"])
                .trim_end()
                .to_string(),
            PULSE_CAP as usize + 5,
        )
        .collect();
        assert_eq!(pulse_from(&flood, now)[PULSE_BUCKETS - 1], PULSE_CAP);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_unreadable_tail_costs_the_pulse_and_the_duration_and_nothing_else() {
        let root = temp_root("pulse-unreadable");
        let path = root.join("projects/-home-j-dev-QuotaPane/deadbeef.jsonl");
        write(
            &path,
            &format!("this is not json at all — {SENTINEL}\nnor is this one\n"),
        );

        let roots = SessionRoots {
            claude: Some(root.clone()),
            codex: None,
        };
        let sessions = scan(&roots, SystemTime::now());
        assert_eq!(sessions.len(), 1, "a row is still produced: {sessions:?}");
        assert_eq!(sessions[0].pulse, [0; PULSE_BUCKETS]);
        assert_eq!(sessions[0].duration, None);
        assert_eq!(sessions[0].turn, TurnState::Unknown);
        assert_eq!(
            sessions[0].state,
            AgentState::Working,
            "mtime still decides"
        );
        assert!(!every_output(&sessions).contains(SENTINEL));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_finished_session_has_no_pulse_at_all() {
        let root = temp_root("pulse-recent");
        let now = at("2026-08-06T12:05:00Z");
        let path = root.join("projects/-home-j-dev-QuotaPane/aaaa1111.jsonl");
        // Records inside the strip's ten minutes, on a file whose mtime says
        // the session ended over an hour ago. Contradictory on purpose: what
        // is being tested is the `Recent` guard, not the arithmetic.
        write(
            &path,
            &claude_log_at(
                "aaaa1111",
                "/home/j/dev/QuotaPane",
                &["2026-08-06T12:04:30Z", "2026-08-06T12:04:59Z"],
            ),
        );
        set_mtime(&path, at("2026-08-06T11:00:00Z"));

        let roots = SessionRoots {
            claude: Some(root.clone()),
            codex: None,
        };
        let sessions = scan(&roots, now);
        assert_eq!(sessions.len(), 1, "{sessions:?}");
        assert_eq!(sessions[0].state, AgentState::Recent);
        assert_eq!(
            sessions[0].pulse, [0; PULSE_BUCKETS],
            "a finished session's rhythm is not computed at all"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    // --------------------------------------------------------- M16: duration

    #[test]
    fn duration_runs_from_the_head_stamp_to_the_last_write() {
        let root = temp_root("duration");
        let now = at("2026-08-06T12:05:00Z");
        let dir = root.join("projects/-home-j-dev-QuotaPane");

        let normal = dir.join("aaaa1111.jsonl");
        write(
            &normal,
            &claude_log_at(
                "aaaa1111",
                "/home/j/dev/QuotaPane",
                &["2026-08-06T11:54:00Z", "2026-08-06T12:04:59Z"],
            ),
        );
        set_mtime(&normal, at("2026-08-06T12:04:59Z"));

        // A head line that is not a record names no instant to measure from.
        let headless = dir.join("bbbb2222.jsonl");
        write(
            &headless,
            &format!(
                "{{\"note\": \"{SENTINEL}\"}}\n{}",
                claude_log_at(
                    "bbbb2222",
                    "/home/j/dev/QuotaPane",
                    &["2026-08-06T12:04:59Z"]
                )
            ),
        );
        set_mtime(&headless, at("2026-08-06T12:04:59Z"));

        // A clock that moved: the head is stamped after the last write.
        let backwards = dir.join("cccc3333.jsonl");
        write(
            &backwards,
            &claude_log_at(
                "cccc3333",
                "/home/j/dev/QuotaPane",
                &["2026-08-06T12:04:59Z"],
            ),
        );
        set_mtime(&backwards, at("2026-08-06T12:00:00Z"));

        let roots = SessionRoots {
            claude: Some(root.clone()),
            codex: None,
        };
        let sessions = scan(&roots, now);
        let duration = |id: &str| {
            sessions
                .iter()
                .find(|s| s.short_id == id)
                .unwrap_or_else(|| panic!("{id} missing from {sessions:?}"))
                .duration
        };
        assert_eq!(
            duration("aaaa1111"),
            Some(Duration::from_secs(659)),
            "11:54:00 to 12:04:59"
        );
        assert_eq!(duration("bbbb2222"), None, "no stamp on the head line");
        assert_eq!(
            duration("cccc3333"),
            None,
            "backwards is None, never a zero that would read as 'just started'"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_tail_read_widens_what_is_counted_and_not_what_is_believed() {
        // The pulse counts every complete line of the tail chunk; the identity
        // still comes from the head and the *last* line only. A middle line
        // claiming a different session must therefore be a beat and nothing
        // more.
        let root = temp_root("tail-belief");
        let now = at("2026-08-06T12:05:00Z");
        let path = root.join("projects/-home-j-dev-QuotaPane/aaaa1111.jsonl");
        let mut log = claude_log_at(
            "aaaa1111",
            "/home/j/dev/QuotaPane",
            &["2026-08-06T12:04:00Z"],
        );
        let middle = serde_json::json!({
            "sessionId": "ffffffff",
            "cwd": "/home/j/dev/somewhere-else",
            "gitBranch": "not-this-one",
            "type": "assistant",
            "timestamp": "2026-08-06T12:04:30Z",
            "message": {"role": "assistant", "content": SENTINEL},
        });
        log.push_str(&format!("{middle}\n"));
        log.push_str(&claude_log_at(
            "aaaa1111",
            "/home/j/dev/QuotaPane",
            &["2026-08-06T12:04:59Z"],
        ));
        write(&path, &log);
        set_mtime(&path, at("2026-08-06T12:04:59Z"));

        let roots = SessionRoots {
            claude: Some(root.clone()),
            codex: None,
        };
        let sessions = scan(&roots, now);
        assert_eq!(sessions.len(), 1, "{sessions:?}");
        assert_eq!(sessions[0].short_id, "aaaa1111", "the head named the row");
        assert_eq!(sessions[0].branch.as_deref(), Some("main"));
        assert_eq!(
            sessions[0].pulse.iter().sum::<u32>(),
            3,
            "every line in the chunk is a beat, including the middle one"
        );
        // The middle line and the tail share the newest minute; the head is a
        // minute older, exactly at the boundary, and boundaries fall back.
        assert_eq!(sessions[0].pulse[PULSE_BUCKETS - 1], 2);
        assert_eq!(sessions[0].pulse[PULSE_BUCKETS - 2], 1);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_tail_chunk_that_starts_mid_line_hands_back_no_fragment() {
        // The tail read starts at a byte offset, not a line boundary, so the
        // first thing in the chunk is normally half of a line that began before
        // the window. Every line handed back has to be a whole line of the
        // file: a fragment that happened to parse would be a record nobody
        // wrote, counted as a beat that never happened.
        let root = temp_root("fragment");
        let path = root.join("projects/-home-j-dev-QuotaPane/aaaa1111.jsonl");
        let head = claude_log_at(
            "aaaa1111",
            "/home/j/dev/QuotaPane",
            &["2026-08-06T12:00:00Z"],
        );
        let tail = claude_log_at(
            "aaaa1111",
            "/home/j/dev/QuotaPane",
            &["2026-08-06T12:04:59Z"],
        );
        // One padding line exactly TAIL_CAP long puts the read's start strictly
        // inside it, whatever the other two lines measure.
        let body = format!("{head}{}\n{tail}", "x".repeat(TAIL_CAP));
        write(&path, &body);

        let (read_head, tail_lines) = read_head_and_tail(&path).expect("the fixture must read");
        assert_eq!(
            read_head,
            head.trim_end(),
            "the head is still the first line"
        );
        let whole_lines: Vec<&str> = body.lines().collect();
        for line in &tail_lines {
            assert!(
                whole_lines.contains(&line.as_str()),
                "not a whole line of the file: {:?}…",
                &line[..line.len().min(48)]
            );
        }
        assert_eq!(
            tail_lines.last().map(String::as_str),
            Some(tail.trim_end()),
            "and the last of them is still the file's last line"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
