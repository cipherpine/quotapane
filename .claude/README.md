# .claude/ — floor-session configuration (§4.1: top tier only)

`settings.json` is the permission posture every floor session runs
under, interactive or headless (the M11d dispatcher). Design:

- **Allow** exactly what the specs need: cargo, non-destructive git
  plus the add/commit/push/tag a spec directs, gh (runs, releases,
  api, attestations), python3 for tools/, the release verifier, and
  ordinary file utilities. File tools are repo-scoped by Claude Code
  itself.
- **Deny** the two things a floor must never do regardless of prompt:
  read credential stores (`~/.claude/**`, `~/.codex/**` — §4.4; the
  product reads those files, sessions never do) and reach the network
  outside gh/cosign (`curl`/`wget`, WebFetch/WebSearch). `claude` is
  denied so a session cannot spawn sessions.
- `--dangerously-skip-permissions` is never used; a headless denial
  is a safe failure the dispatcher logs, not something to bypass.

A tool a spec legitimately needs but the allowlist blocks is a STOP
and a report — the fix is a top-tier edit here, not a workaround.
