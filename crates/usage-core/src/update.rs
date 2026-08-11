//! The update check (M18b) — notify-only, off by default, and unable to
//! carry a credential.
//!
//! SECURITY.md invariant 5 spent three releases as an absence: "no update
//! mechanism exists at all", with an escape clause naming the exact terms any
//! future check would have to meet. The egress module's own comment
//! pre-committed the other half — `api.github.com` was removed from the
//! allowlist in M6 because nothing called it, and was to return "ONLY together
//! with the update-check code itself". This module is both promises coming due
//! at once, which is why it and the allowlist entry land in one commit.
//!
//! ## What it does, exactly
//!
//! One anonymous `GET https://api.github.com/repos/cipherpine/quotapane/`
//! `releases/latest` through the existing chokepoint, from which exactly one
//! field is read: `tag_name`. If that tag parses as `vX.Y.Z` and is strictly
//! newer than this build, the caller gets a version string and a URL to show.
//! Everything else is [`None`].
//!
//! ## What it cannot do
//!
//! - **It cannot authenticate.** There is no parameter, field, or header here
//!   through which a caller could attach credential material, and the request
//!   is built with the chokepoint's `bearer` argument set to [`None`]. A test
//!   scans this module's own source for the names such a thing would need.
//! - **It cannot identify you.** The only header sent is a static
//!   `User-Agent: quotapane-update-check` — no version string, no OS, no
//!   machine or account identifier. GitHub rejects requests without a
//!   `User-Agent` at all, so this is the minimum disclosure that works, and it
//!   is the same eleven bytes from every install on earth.
//! - **It cannot report *why* it failed.** No error type escapes this module —
//!   there is no error type in it at all. A refused request, a timeout, a 500,
//!   a body that is not JSON and a tag that does not parse all collapse into
//!   one payload-free [`CheckOutcome::Inconclusive`], and [`check`] — the
//!   window's entry point — flattens even that into [`None`]. The window must
//!   be silent when the network is unhappy. The CLI, which the user invoked on
//!   purpose, is allowed to say "that did not work" and exit non-zero, but it
//!   cannot say more than that, because there is nothing more to say: a
//!   failure detail here would be the one place a proxy variable's name or a
//!   URL could reach a screen.
//! - **It cannot run unasked.** [`check`] takes the stored preference and
//!   returns before building anything unless it is explicitly `on`. Absent —
//!   the default, and the state a fresh install is in — sends nothing.
//! - **It cannot remember.** Nothing is cached, no file is written, no state
//!   is kept. The window checks once per launch and a window left running for
//!   a week never checks again: restarting *is* the re-check. That is a
//!   deliberate simplicity — a timer would need state, and state here would be
//!   the first thing to grow a cache file.
//!
//! ## Reading only `tag_name`
//!
//! The release payload also carries release notes, asset URLs, an author, and
//! whatever GitHub adds next. None of it is deserialized into anything: the
//! body is parsed as a generic JSON value and exactly one string is lifted out
//! of it. That is the same welded-aperture discipline [`crate::agents`] uses
//! for session logs — parse only what you print — and it is why a hostile or
//! merely surprising response has nowhere to land.

use crate::egress::Egress;

/// Where a newer release lives, as shown to a human.
///
/// Scheme-less on purpose: it is display text — a hover tooltip and one line
/// of CLI output — not something the app ever fetches or opens.
pub const RELEASES_URL: &str = "github.com/cipherpine/quotapane/releases";

/// The one host this module contacts. On the egress allowlist for this module
/// and no other (pinned by [`tests::update_is_the_only_caller_of_the_github_host`]).
const HOST: &str = "api.github.com";

/// The latest-release endpoint for this repository.
const PATH: &str = "/repos/cipherpine/quotapane/releases/latest";

/// Sent verbatim from every install. GitHub refuses requests with no
/// `User-Agent`; this is the least that satisfies it and says nothing about
/// the machine it came from.
const USER_AGENT: &str = "quotapane-update-check";

/// How much of the response body is looked at.
///
/// The chokepoint already caps bodies at 1 MiB; this is the tighter cap for a
/// document from which one short string is read. A body longer than this is
/// truncated rather than rejected, and a truncated document simply fails to
/// parse — which is [`None`], like every other disappointment here.
const MAX_BODY_BYTES: usize = 64 * 1024;

/// A newer release, ready to render.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateNotice {
    /// The newer version as the tag named it, `v` included — the string the
    /// window and the CLI print, and the only text from the response that ever
    /// reaches a screen.
    pub version: String,
    /// Where to get it: [`RELEASES_URL`], carried along so a caller never has
    /// to know the constant's name.
    pub url: &'static str,
}

/// A semantic version, compared as numbers rather than as text — `v1.10.0` is
/// newer than `v1.9.0`, which a string comparison gets backwards.
type Version = (u64, u64, u64);

/// Whether a check may run at all — **the gate**.
///
/// `None` is the un-asked state a fresh install is in, and it means the same
/// thing as `off`: send nothing. Only an explicit `Some(true)` opens the door.
/// Written as its own named function, and routed through by both callers, so
/// that "off by default" is one testable expression rather than a condition
/// repeated at two call sites with a chance of drifting apart.
pub fn opted_in(setting: Option<bool>) -> bool {
    matches!(setting, Some(true))
}

/// What a check concluded. Three states, no payload on two of them, and
/// deliberately **no error type** — see the module docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
    /// A strictly newer release exists.
    Newer(UpdateNotice),
    /// This build is the latest one published.
    Current,
    /// Nothing was learned. Either the check was not asked for, or it did not
    /// complete — and those are the same answer, because neither is a claim
    /// that you are current. Carries nothing: there is no detail here to leak,
    /// by construction rather than by discipline.
    Inconclusive,
}

/// Check for a newer release, if and only if the user asked for one.
///
/// The window's entry point, and the shape SECURITY.md invariant 5 describes:
/// `Some` only when the latest tag parses and is strictly newer than this
/// build; everything else — not opted in, refused by the chokepoint, a
/// transport failure, a non-200, an unparseable body, an unparseable tag, an
/// equal or older version — is `None`, indistinguishable from one another.
///
/// A caller that is allowed to be honest about failure (the CLI, which the
/// user invoked deliberately) uses [`check_outcome`] instead.
pub fn check(egress: &Egress, setting: Option<bool>) -> Option<UpdateNotice> {
    match check_outcome(egress, setting) {
        CheckOutcome::Newer(notice) => Some(notice),
        CheckOutcome::Current | CheckOutcome::Inconclusive => None,
    }
}

/// Check for a newer release, distinguishing "you are current" from "I could
/// not tell" — and nothing finer than that.
///
/// The gate is here, ahead of everything, so both entry points and therefore
/// both callers pass through [`opted_in`] exactly once.
pub fn check_outcome(egress: &Egress, setting: Option<bool>) -> CheckOutcome {
    if !opted_in(setting) {
        return CheckOutcome::Inconclusive;
    }
    let Some(tag) = latest_tag(egress) else {
        return CheckOutcome::Inconclusive;
    };
    match notice_if_newer(&tag, env!("CARGO_PKG_VERSION")) {
        Some(notice) => CheckOutcome::Newer(notice),
        // The tag was read and is not newer. `notice_if_newer` also returns
        // `None` for a tag that does not parse, which is not the same thing —
        // so that case is separated out here rather than reported as current.
        None if tag_version(&tag).is_some() => CheckOutcome::Current,
        None => CheckOutcome::Inconclusive,
    }
}

/// Fetch the latest release tag. The only place in this workspace that names
/// [`HOST`], and the only request this module makes.
fn latest_tag(egress: &Egress) -> Option<String> {
    let response = egress
        .get(HOST, PATH, None, &[("User-Agent", USER_AGENT)])
        .ok()?;
    if response.status != 200 {
        return None;
    }
    let body = &response.body[..response.body.len().min(MAX_BODY_BYTES)];
    tag_name(body)
}

/// Lift `tag_name` out of a release document, and nothing else.
///
/// The whole aperture, in one function: the body becomes a generic JSON value,
/// one key is read from the top level, and it must be a string. There is no
/// struct here for a field to be added to by accident.
fn tag_name(body: &[u8]) -> Option<String> {
    let document: serde_json::Value = serde_json::from_slice(body).ok()?;
    Some(document.get("tag_name")?.as_str()?.to_string())
}

/// Split `X.Y.Z` into three numbers. Anything else — extra components, a
/// missing one, a suffix, a non-number — is `None`, and therefore silence.
fn triple(text: &str) -> Option<Version> {
    let mut parts = text.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// Parse a release tag, which this project has always written as `vX.Y.Z`.
///
/// The `v` is required rather than optional: the shape is a fact about this
/// repository's own tags, and a tag that does not match it is drift the check
/// should fail closed on instead of guessing at.
fn tag_version(tag: &str) -> Option<Version> {
    triple(tag.strip_prefix('v')?)
}

/// Compare a fetched tag against a running version, both as numbers.
///
/// Separated from [`check`] so the comparison is testable without a network:
/// this is where "strictly newer" lives, and it is the one place a flipped
/// inequality would announce a downgrade as an upgrade.
fn notice_if_newer(tag: &str, running: &str) -> Option<UpdateNotice> {
    let latest = tag_version(tag)?;
    let current = triple(running)?;
    if latest > current {
        Some(UpdateNotice {
            version: tag.to_string(),
            url: RELEASES_URL,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This module's real code: everything above the test module, with comment
    /// lines dropped. The doc comment necessarily discusses the very names the
    /// scans below forbid, so it has to go before they run.
    fn real_code() -> String {
        const SRC: &str = include_str!("update.rs");
        SRC[..SRC
            .find("#[cfg(test)]")
            .expect("test module marker not found")]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// SECURITY.md invariant 5: the check is off by default and sends nothing
    /// until it is switched on.
    ///
    /// Both halves, because either alone is weak. Behaviourally: the two
    /// un-asked states return `None`, and they return it fast — a real request
    /// to a real host could not complete in the time this test takes.
    /// Structurally: the gate is the first thing in `check`, ahead of the only
    /// `egress.get(` in the module, so "no request is constructed" is a
    /// property of the code's shape and not of a lucky early return.
    // INV:5 — registered in invariants.manifest (checked in CI)
    #[test]
    fn the_update_check_sends_nothing_unless_asked() {
        let egress = Egress::new(false);

        // Absent — a fresh install, the question never put to the user.
        assert_eq!(check(&egress, None), None);
        // Explicitly declined.
        assert_eq!(check(&egress, Some(false)), None);

        // Both entry points, because the CLI uses the other one: an un-asked
        // check concludes nothing, and in particular does not claim you are
        // current — a claim it has not earned without sending anything.
        assert_eq!(check_outcome(&egress, None), CheckOutcome::Inconclusive);
        assert_eq!(
            check_outcome(&egress, Some(false)),
            CheckOutcome::Inconclusive
        );

        // The gate itself, as a truth table. Only one input opens it.
        assert!(!opted_in(None), "an unanswered question is not consent");
        assert!(!opted_in(Some(false)));
        assert!(opted_in(Some(true)));

        // The shape: the gate is consulted and returns before the request
        // exists. A mutation that moves the gate below the fetch, or drops it,
        // lands here even if it somehow still returned `None` in this process.
        let code = real_code();
        let body = code
            .split("pub fn check_outcome(")
            .nth(1)
            .expect("check_outcome() not found");
        let gate = body
            .find("if !opted_in(setting)")
            .expect("the gate is gone");
        let fetch = body.find("latest_tag(egress)").expect("the fetch moved");
        assert!(
            gate < fetch,
            "the opt-in gate must precede the request, not follow it"
        );
        assert!(
            body[gate..fetch].contains("return CheckOutcome::Inconclusive;"),
            "the gate must return before the fetch, not merely sit above it"
        );
        // And the window's entry point cannot reach the network around it: it
        // is a pure mapping over the gated one.
        let window_entry = code
            .split("pub fn check(")
            .nth(1)
            .and_then(|rest| rest.split("\npub fn ").next())
            .expect("check() not found");
        assert!(
            !window_entry.contains("latest_tag("),
            "check() must go through check_outcome(), not around it"
        );
        // And the request itself is built in exactly one place.
        assert_eq!(
            code.matches(".get(HOST").count(),
            1,
            "the update check must make exactly one request, in one place"
        );
    }

    /// SECURITY.md invariant 5: the request cannot carry a credential.
    ///
    /// Not "does not" — *cannot*. The module is scanned for every name the
    /// machinery would need, the same technique the statusline mode uses to
    /// prove it sends nothing, and the public API is inspected for a parameter
    /// one could be passed through. The chokepoint's `bearer` argument is the
    /// only route a token could take out of this process, and this module
    /// passes it `None` at its single call site.
    // INV:5 — registered in invariants.manifest (checked in CI)
    #[test]
    fn the_update_request_cannot_carry_a_credential() {
        let code = real_code();

        // Guard the scanner: an empty slice would pass everything vacuously.
        assert!(
            code.contains("pub fn check("),
            "the scanner lost the module body"
        );

        for forbidden in [
            "Authorization",
            "Bearer",
            "bearer",
            "Secret",
            "token",
            "Token",
            "credential",
            "Credential",
            "auth",
            "Auth",
        ] {
            assert!(
                !code.contains(forbidden),
                "the update module must not name `{forbidden}` — it is the anonymous path"
            );
        }

        // The single request passes `None` where a credential would go. Written
        // as a literal at the call site, so there is no variable a future edit
        // could quietly point at something else.
        assert!(
            code.contains(".get(HOST, PATH, None, &[(\"User-Agent\", USER_AGENT)])"),
            "the request must be built with no credential and the static User-Agent"
        );

        // The User-Agent is a constant, not a format string: nothing about this
        // machine can be interpolated into the one header we do send.
        assert!(
            code.contains("const USER_AGENT: &str = \"quotapane-update-check\";"),
            "the User-Agent must be a static constant"
        );
        // The running version is read once, for the local comparison, and the
        // function that builds the request never sees it. Both halves matter:
        // the count keeps a second reader from appearing, and the scope check
        // keeps this one from drifting into the request.
        assert_eq!(
            code.matches("env!(\"CARGO_PKG_VERSION\")").count(),
            1,
            "the running version is for the local comparison only"
        );
        let request_builder = code
            .split("fn latest_tag(")
            .nth(1)
            .and_then(|rest| rest.split("\nfn ").next())
            .expect("latest_tag not found");
        assert!(
            !request_builder.contains("CARGO_PKG_VERSION"),
            "the request must not learn the running version"
        );
    }

    /// SECURITY.md invariant 5 / invariant 3: exactly one expression in the
    /// workspace names the GitHub host, and it is this module's constant.
    ///
    /// The allowlist entry and its caller were added together on the egress
    /// module's own rule; this is what keeps them together. A second call site
    /// anywhere in the workspace — a well-meaning "just fetch the release
    /// notes too" in the UI — fails here, because the whole argument for
    /// re-adding the host was that exactly one audited function reaches it.
    ///
    /// Same shape as the `refresh_agents` single-call-site pin, widened from
    /// one file to the whole source tree: the allowlist is a workspace-level
    /// promise, so a file-local scan would be the wrong aperture.
    // INV:5 — registered in invariants.manifest (checked in CI)
    #[test]
    fn update_is_the_only_caller_of_the_github_host() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();

        let mut sources = Vec::new();
        collect_rust_sources(&root.join("crates"), &mut sources);
        assert!(
            sources.len() >= 4,
            "the source walk found almost nothing: {sources:?}"
        );

        let mut naming = Vec::new();
        for path in &sources {
            // The allowlist itself must name the host — that is its job — and
            // the rejection fixtures next to it name smuggling variants of it.
            if path.ends_with("egress/mod.rs") {
                continue;
            }
            let text = std::fs::read_to_string(path).expect("unreadable source");
            // Tests may name it freely; this is a claim about shipped code.
            let code = match text.find("#[cfg(test)]") {
                Some(at) => &text[..at],
                None => &text[..],
            };
            let hits = code
                .lines()
                .filter(|line| !line.trim_start().starts_with("//"))
                .filter(|line| line.contains("api.github.com"))
                .count();
            if hits > 0 {
                naming.push((path.clone(), hits));
            }
        }

        assert_eq!(
            naming.len(),
            1,
            "exactly one file may name the GitHub host outside egress and tests, found: {naming:?}"
        );
        let (path, hits) = &naming[0];
        assert!(
            path.ends_with("update.rs"),
            "the only caller of the GitHub host must be update.rs, found {path:?}"
        );
        assert_eq!(
            *hits, 1,
            "the host belongs in exactly one expression: the HOST constant"
        );
    }

    /// Every `.rs` under a directory, recursively.
    fn collect_rust_sources(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // `target/` can appear inside a crate directory and holds
                // generated copies of nothing this test is asking about.
                if path.file_name().is_some_and(|n| n == "target") {
                    continue;
                }
                collect_rust_sources(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }

    // --- the comparison ---

    #[test]
    fn a_strictly_newer_tag_is_the_only_thing_that_notifies() {
        let notice = notice_if_newer("v1.8.0", "1.7.0").expect("1.8.0 is newer than 1.7.0");
        assert_eq!(notice.version, "v1.8.0");
        assert_eq!(notice.url, RELEASES_URL);

        // Equal and older say nothing at all.
        assert_eq!(notice_if_newer("v1.7.0", "1.7.0"), None);
        assert_eq!(notice_if_newer("v1.6.9", "1.7.0"), None);
        assert_eq!(notice_if_newer("v0.9.9", "1.7.0"), None);
    }

    #[test]
    fn versions_compare_as_numbers_not_as_text() {
        // The case a string comparison gets backwards, in both directions.
        assert!(notice_if_newer("v1.10.0", "1.9.0").is_some());
        assert_eq!(notice_if_newer("v1.9.0", "1.10.0"), None);
        assert!(notice_if_newer("v2.0.0", "1.999.999").is_some());
        assert!(notice_if_newer("v1.7.10", "1.7.9").is_some());
        assert_eq!(notice_if_newer("v1.7.9", "1.7.10"), None);
    }

    #[test]
    fn an_unparseable_tag_is_silence_rather_than_a_guess() {
        for tag in [
            "",
            "v",
            "1.7.1",         // no leading v — this project's tags have one
            "v1.8",          // too few components
            "v1.8.0.1",      // too many
            "v1.8.0-rc1",    // a prerelease suffix is not a release
            "release-1.8.0", // a different tag scheme entirely
            "v1.8.x",        // not a number
            "vNaN.0.0",
            "v-1.0.0",
            "v99999999999999999999.0.0", // overflows u64
        ] {
            assert_eq!(
                notice_if_newer(tag, "1.7.0"),
                None,
                "{tag:?} must not produce a notice"
            );
        }
    }

    #[test]
    fn the_running_version_is_this_build() {
        // The comparison's other operand is the crate's own version, so a
        // release that forgot to bump would compare against the real number.
        assert!(triple(env!("CARGO_PKG_VERSION")).is_some());
        // And this build is not newer than itself.
        assert_eq!(
            notice_if_newer(
                &format!("v{}", env!("CARGO_PKG_VERSION")),
                env!("CARGO_PKG_VERSION")
            ),
            None
        );
    }

    // --- the aperture ---

    #[test]
    fn only_tag_name_is_read_out_of_a_release_document() {
        // A realistic payload with a sentinel in every other field GitHub
        // sends. Only the tag may come back.
        const SENTINEL: &str = "SENTINEL-DO-NOT-READ";
        let body = format!(
            r#"{{
  "url": "https://{SENTINEL}.example/releases/1",
  "html_url": "https://{SENTINEL}.example/releases/tag/v1.8.0",
  "tag_name": "v1.8.0",
  "name": "{SENTINEL}-release-name",
  "body": "{SENTINEL} release notes with an install script in them",
  "author": {{"login": "{SENTINEL}-author", "id": 1}},
  "assets": [
    {{"browser_download_url": "https://{SENTINEL}.example/a.zip", "name": "{SENTINEL}.zip"}}
  ],
  "tarball_url": "https://{SENTINEL}.example/t.tar.gz"
}}"#
        );

        let tag = tag_name(body.as_bytes()).expect("the document has a tag_name");
        assert_eq!(tag, "v1.8.0");
        assert!(!tag.contains(SENTINEL));

        // And through the comparison, which is what actually reaches a screen.
        let notice = notice_if_newer(&tag, "1.7.0").expect("newer");
        let rendered = format!("{} {}", notice.version, notice.url);
        assert!(
            !rendered.contains(SENTINEL),
            "a field outside tag_name reached the output: {rendered}"
        );
    }

    #[test]
    fn a_body_that_is_not_a_release_document_is_silence() {
        for body in [
            "",
            "   ",
            "not json",
            "{",
            "null",
            "[]",
            "42",
            r#"{"message": "Not Found"}"#, // GitHub's 404 body
            r#"{"tag_name": null}"#,
            r#"{"tag_name": 180}"#,          // a number, not a string
            r#"{"tag_name": {"v": "1.8"}}"#, // an object
            r#"{"TAG_NAME": "v1.8.0"}"#,     // JSON keys are case-sensitive
        ] {
            assert_eq!(
                tag_name(body.as_bytes()),
                None,
                "{body:?} must not yield a tag"
            );
        }
    }

    #[test]
    fn a_body_truncated_at_the_cap_simply_fails_to_parse() {
        // The cap is a defence, not a parser: a document longer than it is cut
        // mid-JSON, and a cut document is `None` like any other malformed one.
        // Proven on the real boundary rather than described.
        let padding = "x".repeat(MAX_BODY_BYTES);
        let oversized = format!(r#"{{"note": "{padding}", "tag_name": "v1.8.0"}}"#);
        let body = oversized.as_bytes();
        assert!(body.len() > MAX_BODY_BYTES);

        let truncated = &body[..body.len().min(MAX_BODY_BYTES)];
        assert_eq!(
            tag_name(truncated),
            None,
            "a truncated document must not parse"
        );
        // The same document under the cap reads normally, so the test above is
        // measuring the cap and not a broken fixture.
        assert_eq!(
            tag_name(br#"{"note": "short", "tag_name": "v1.8.0"}"#),
            Some("v1.8.0".to_string())
        );
    }

    /// "Current" is a claim; "inconclusive" is the absence of one. The three
    /// outcomes exist so the CLI can tell those apart, and the boundary between
    /// them is here: a tag we could not read is never reported as up to date.
    #[test]
    fn a_tag_we_could_not_read_is_never_reported_as_current() {
        // The mapping `check_outcome` applies once a tag is in hand, exercised
        // through the same two functions it uses.
        let classify = |tag: &str| match notice_if_newer(tag, "1.7.0") {
            Some(notice) => CheckOutcome::Newer(notice),
            None if tag_version(tag).is_some() => CheckOutcome::Current,
            None => CheckOutcome::Inconclusive,
        };

        assert!(matches!(classify("v1.8.0"), CheckOutcome::Newer(_)));
        assert_eq!(classify("v1.7.0"), CheckOutcome::Current);
        assert_eq!(classify("v1.6.0"), CheckOutcome::Current);
        // Drift in the tag scheme is not evidence of being up to date.
        assert_eq!(classify("release-1.8.0"), CheckOutcome::Inconclusive);
        assert_eq!(classify("v1.8.0-rc1"), CheckOutcome::Inconclusive);
        assert_eq!(classify(""), CheckOutcome::Inconclusive);
    }

    /// The module exposes no error type — not a struct, not an enum, and
    /// nothing implementing `std::error::Error`. The CLI's "it failed" is the
    /// entire vocabulary, and it carries no payload to leak a URL, a host, or
    /// a proxy variable's name into.
    ///
    /// Deliberately **not** tagged `// INV:5`: `invariants.manifest` is a
    /// protected path whose M18b entry was authored at the top tier and lands
    /// verbatim, and the checker holds tags and manifest set-equal in both
    /// directions. An extra tag here would fail that check, and adding the
    /// matching line would be this session authoring protected bytes. The test
    /// guards the module either way.
    #[test]
    fn no_error_type_escapes_this_module() {
        let code = real_code();
        for forbidden in [
            "Error",
            "Result<",
            "std::error",
            "thiserror",
            "anyhow",
            "panic!",
            "unwrap()",
            "expect(",
        ] {
            assert!(
                !code.contains(forbidden),
                "the update module must not name `{forbidden}` — it reports one payload-free outcome"
            );
        }
        // The one carrier of information out of here is a version string that
        // came from a tag, plus a compile-time constant URL.
        assert!(
            code.contains("pub version: String,"),
            "the notice's only dynamic field is the version"
        );
        assert!(
            code.contains("pub url: &'static str,"),
            "the URL must be a compile-time constant, not built from a response"
        );
    }

    #[test]
    fn the_notice_carries_the_releases_url_and_no_scheme() {
        let notice = notice_if_newer("v9.9.9", "1.7.0").expect("newer");
        assert_eq!(notice.url, "github.com/cipherpine/quotapane/releases");
        // Display text, not something to fetch: no scheme, and the app never
        // opens it — there is no click handler anywhere in the window.
        assert!(!notice.url.starts_with("http"));
    }

    #[test]
    fn the_endpoint_is_the_latest_release_of_this_repository() {
        assert_eq!(HOST, "api.github.com");
        assert_eq!(PATH, "/repos/cipherpine/quotapane/releases/latest");
        // The chokepoint requires a leading slash; a path without one is how
        // an authority could be smuggled into the composed URL.
        assert!(PATH.starts_with('/'));
        Egress::check_host(HOST).expect("the update host must be on the allowlist");
    }
}
