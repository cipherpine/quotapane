## What & why

<!-- What does this change, and what problem does it solve? -->

## Checklist

- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and `cargo test --workspace --locked` all pass locally.
- [ ] This PR does **not** touch a protected path (`crates/usage-core/src/egress/**`, `crates/usage-core/src/credentials/**`, any security-invariant test, `deny.toml`, `SECURITY.md`, `THREAT_MODEL.md`, `.github/**`, `.cargo/**`) — or, if it must, I have said so explicitly below and expect maintainer review of every byte.
- [ ] Any `Cargo.lock` change is a justified dependency change with a matching row in `CONTRIBUTING.md`'s table, not incidental churn.

## Security invariant statement (required)

<!-- Name the SECURITY.md invariant (1–7) your change could most plausibly
     affect, and say in one or two sentences why it does not weaken it.
     "None plausibly affected" is a valid answer for pure-UI/docs changes —
     but say it, don't skip it. -->
