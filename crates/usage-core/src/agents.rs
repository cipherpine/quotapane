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

/// **The complete set of JSON keys this module may read.** SECURITY.md
/// invariant 8 is exactly this list plus [`read_allowlisted`].
///
/// Claude Code writes `sessionId`, `timestamp`, `type`, `cwd`, `gitBranch` and
/// `isSidechain` on its record lines; the Codex CLI's `session_meta` record
/// carries `id`, `cwd` and `git_branch`, wrapped in a record that carries its
/// own `timestamp` and `type`. Every one of them is an identifier, an instant,
/// a record type, a directory, or a branch name. None of them is content, and
/// no key outside this list is readable from here at all: [`read_allowlisted`]
/// checks membership before it will hand back a value, so deleting an entry
/// below removes a capability rather than merely a mention.
pub const ALLOWLISTED_KEYS: &[&str] = &[
    // Claude Code record lines.
    "sessionId",
    "timestamp",
    "type",
    "cwd",
    "gitBranch",
    "isSidechain",
    // Codex CLI `session_meta` payload.
    "id",
    "git_branch",
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

/// One local agent session, identified **without reading a word of it**.
///
/// Every field is either an identifier, a path component, a branch name, or a
/// time. There is deliberately no field of a type that could hold a sentence.
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
/// The search is bounded to two levels — the object itself and any object it
/// holds directly — because the Codex CLI wraps its `session_meta` fields in a
/// `payload` object, and naming that wrapper here would be one more key this
/// module knows how to open. Only scalars are returned: an allowlisted key
/// whose value is an object or an array is treated as absent, so no container
/// can be dragged out whole and formatted somewhere else.
fn read_allowlisted<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    if !ALLOWLISTED_KEYS.contains(&key) {
        return None;
    }
    let object = value.as_object()?;
    let scalar = |v: &'a serde_json::Value| (!v.is_object() && !v.is_array()).then_some(v);
    if let Some(found) = object.get(key).and_then(scalar) {
        return Some(found);
    }
    object
        .values()
        .filter_map(|nested| nested.as_object())
        .find_map(|nested| nested.get(key).and_then(scalar))
}

/// An allowlisted key's value as a non-empty string.
fn read_str<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    let text = read_allowlisted(value, key)?.as_str()?;
    (!text.is_empty()).then_some(text)
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
        let (id_key, branch_key) = match provider {
            ProviderId::ClaudeSubscription => ("sessionId", "gitBranch"),
            // The Codex ids and paths live in a `session_meta` record; an event
            // record carries none of them, and its `id`-shaped keys, if a later
            // version grows any, are not this session's identity.
            ProviderId::CodexSubscription => {
                if read_str(&value, "type") != Some("session_meta") {
                    return;
                }
                ("id", "git_branch")
            }
        };
        if self.id.is_none() {
            self.id = read_str(&value, id_key).map(short_id);
        }
        if self.project.is_none() {
            self.project = read_str(&value, "cwd").and_then(basename);
        }
        if self.branch.is_none() {
            self.branch = read_str(&value, branch_key).map(str::to_string);
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

/// Read a candidate's first line and last line, each bounded by [`TAIL_CAP`].
///
/// `None` for a file that cannot be opened or read — which is not an error
/// here, only the absence of a name to go with a state.
fn read_head_and_tail(path: &Path) -> Option<(String, String)> {
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
        head_text.lines().last().unwrap_or_default().to_string()
    } else {
        file.seek(SeekFrom::Start(len - cap)).ok()?;
        let mut tail_bytes = Vec::new();
        file.read_to_end(&mut tail_bytes).ok()?;
        // The chunk starts mid-line; only the last complete line is used.
        String::from_utf8_lossy(&tail_bytes)
            .lines()
            .last()
            .unwrap_or_default()
            .to_string()
    };
    Some((head, tail))
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
            if let Some((head, tail)) = read_head_and_tail(&candidate.path) {
                extracted.absorb(candidate.provider, &head);
                if tail != head {
                    extracted.absorb(candidate.provider, &tail);
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

    /// Every string a session can show, plus its `Debug` — the whole reachable
    /// output surface of the public types (neither has a `Display` impl).
    fn every_output(sessions: &[AgentSession]) -> String {
        let mut out = format!("{sessions:?}");
        for session in sessions {
            out.push_str(&session.short_id);
            out.push_str(&session.project);
            out.push_str(session.branch.as_deref().unwrap_or_default());
            out.push_str(&format!("{:?}{:?}", session.provider, session.state));
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
}
