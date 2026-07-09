# Contributing

Thanks for helping. This project's headline feature is a **small, auditable trust boundary**, so contributions are judged first on whether they keep that surface small and honest.

## Ground rules

1. **Read `SECURITY.md` and `THREAT_MODEL.md` first.** The seven invariants are binding. A change that weakens one is a breaking security change: call it out explicitly in the PR description and update the docs in the same PR.
2. **The trust boundary stays tiny.** Changes to `crates/usage-core/src/credentials/` or `crates/usage-core/src/egress/` get the strictest review. If your change grows the sensitive surface, expect to be asked for an alternative.
3. **No new dependencies without justification.** Add a row to the table below in the same PR. `cargo-deny` and `cargo-audit` must stay green. Prefer std.
4. **No secrets in the repo, ever.** Test fixtures are synthetic and generated at test runtime. CI runs secret scanning; don't make it earn its keep.
5. **Every security invariant keeps a test.** Use the traceability table in `THREAT_MODEL.md` §9; if you touch an enforcing module, the corresponding test must still pass (and grow if behavior grew).
6. **Threat-model review triggers** (`THREAT_MODEL.md` §11): adding a provider or data source, changing the egress allowlist, changing the update mechanism, or adding a dependency with network or serialization capability all require re-checking §6/§9 in the same PR.

## Why each dependency exists

| Crate | Used by | Why | Why not std |
|---|---|---|---|
| `zeroize` | `usage-core` | Wipes secret bytes from memory on drop (invariant 2). Used without its `derive` feature so syn/quote/proc-macro2 stay out of the trust boundary's tree. | Reliable volatile memory wiping is subtle (compiler may elide naive writes); this is the ecosystem-standard, audited solution. |

(That's the whole tree today. Keep it that way where possible.)

## Development

```sh
cargo test --workspace --locked   # all tests, including invariant tests
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

CI runs the same, plus `cargo-deny`, `cargo-audit`, and a no-telemetry check, on Windows/macOS/Linux.

## Reporting security issues

**Not** via public issues — see `SECURITY.md` for the private disclosure channel and our response commitments.
