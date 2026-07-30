//! Normalized, **non-secret** data types.
//!
//! Everything that crosses from the poller to the UI/CLI is one of these
//! types. None of them can carry a credential: that is enforced by
//! construction (no secret-typed fields), which is what makes the
//! process → UI channel a non-boundary (THREAT_MODEL.md §4).
//!
//! These types derive `Serialize` (for `quotapane-cli --json`). That is safe
//! **because** they hold no secrets — the exact inverse of `Secret<T>`,
//! which forbids `Serialize` for the same reason. Adding a secret-bearing
//! field to any type here would be a breaking security change and must be
//! rejected in review.

use serde::Serialize;

/// Identifies a usage data source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    /// Claude subscription quota (OAuth usage endpoint + header fallback).
    ClaudeSubscription,
    /// OpenAI Codex subscription quota. (M3 — endpoint verified then, not guessed.)
    CodexSubscription,
}

/// A quota window (e.g. Claude "5-hour" or "weekly"), normalized.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct QuotaWindow {
    /// Human-readable window label, e.g. `"5h"` or `"week"`.
    pub label: String,
    /// Fraction of the window consumed, `0.0..=1.0`, when known.
    pub used_fraction: Option<f64>,
    /// Seconds until the window resets, when known.
    pub resets_in_secs: Option<u64>,
    /// The window's **total** length in seconds, when known — e.g. `18000` for
    /// a five-hour window, `604800` for a weekly one (M8).
    ///
    /// Paired with [`Self::resets_in_secs`] this gives how far *through* the
    /// window the account is, which is what makes a pace comparison possible:
    /// `1 - resets_in / duration` is the elapsed fraction, and a
    /// [`Self::used_fraction`] ahead of it means burning faster than time.
    ///
    /// `Option` because the two providers know it differently and neither
    /// always does: Codex states it outright (`limit_window_seconds`), Claude
    /// implies it in the *kind* of limit being reported, and a kind this crate
    /// has not seen before yields `None` rather than a guess. Deliberately not
    /// derived from [`Self::label`] — a label is a display string (a Codex
    /// per-model row's label is a model name), and parsing one back into a
    /// duration is exactly the label-parsing this crate refuses to do.
    pub duration_secs: Option<u64>,
}

/// A snapshot of one provider's state at one poll. Contains no secrets,
/// by type: adding a secret-bearing field here is a breaking security change.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProviderSnapshot {
    /// Which provider this snapshot describes.
    pub provider: ProviderId,
    /// Unix timestamp (seconds) when the poll completed.
    pub taken_at_unix_secs: u64,
    /// Quota windows reported by the provider.
    ///
    /// These are the *headline* windows — the small set that describes the
    /// subscription as a whole (e.g. Claude's `5h` and `7d`). Anything scoped
    /// to a single model belongs in [`Self::per_model`] instead.
    pub windows: Vec<QuotaWindow>,
    /// Per-model quota windows, when the provider reports any.
    ///
    /// Deliberately the same [`QuotaWindow`] type as [`Self::windows`]: a
    /// per-model row *is* a quota window, just scoped to one model, so it
    /// renders through the same code path and needs no parallel struct. The
    /// `label` carries the provider's own model name verbatim (e.g.
    /// `"7d-opus"`, `"GPT-5.3-Codex-Spark"`) — no label parsing is done or
    /// implied anywhere downstream.
    ///
    /// Empty is the normal case for a provider that reports no per-model
    /// data, which is why there is no `Option` wrapper: "none reported" and
    /// "none exist" are the same thing to every consumer.
    pub per_model: Vec<QuotaWindow>,
    /// Reset credits the provider reports, when it reports any.
    ///
    /// `Option` rather than a default-zero value because "this provider has no
    /// such concept" and "this account has zero credits" are different facts
    /// and must render differently: Claude has no equivalent at all, so it
    /// sets `None` and shows nothing, while a genuine zero would still be a
    /// line worth printing.
    pub reset_credits: Option<ResetCredits>,
    /// Whether the data came from the primary endpoint or a fallback.
    pub source: SnapshotSource,
}

/// Reset credits: a provider-granted allowance for clearing a rate limit
/// early. Codex reports these; Claude has no equivalent.
///
/// Two counts, because the provider distinguishes owning a credit from being
/// able to spend one: `applicable_now` is normally `0` while under no rate
/// limit, and only becomes non-zero when a credit could actually be applied.
/// Collapsing them into one number would misreport an account that owns
/// credits it cannot currently use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ResetCredits {
    /// Credits the account owns.
    pub available: u32,
    /// Credits usable right now, when the provider says.
    pub applicable_now: Option<u32>,
}

/// Where a snapshot's data came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotSource {
    /// The provider's usage endpoint (may be undocumented — disclaimed at runtime).
    UsageEndpoint,
    /// Rate-limit headers from an official API response (fallback).
    RateLimitHeaders,
}
