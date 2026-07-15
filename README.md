# QuotaPane

> **Working name** — will be renamed before the first public release.
> Status: **M0 — trust boundary & scaffolding.** Not yet useful on a desk; the security core ships before the features do, on purpose.

A live, always-on-top desktop window showing your AI usage and quota across **Anthropic (Claude)** and **OpenAI (Codex)**, read locally from **your own** credentials. Single Rust binary, no web layer, no telemetry, no auto-update.

The entire value proposition is a **small, auditable trust boundary**: the only code that touches credentials or the network lives in two small modules you can read end to end — `crates/usage-core/src/credentials/` and `crates/usage-core/src/egress/`. Everything else is scheduling and rendering.

## Security posture (the short version)

- Tokens are never persisted, never logged, never serialized; they live in memory in a `Secret<T>` that zeroizes on drop and prints `«redacted»`.
- Network egress is deny-by-default through a single chokepoint with a compile-time host allowlist. A non-allowlisted host is a hard error, and a test proves it.
- No first-party telemetry (CI enforces its absence). No silent auto-update — the optional update *check* only notifies, and it's off by default.
- Credential files are opened read-only; token refresh is delegated to the official `claude` / `codex` CLIs.
- Proxy support is opt-in, with an explicit warning that a TLS-inspecting proxy can observe bearer tokens.

Full detail: [`SECURITY.md`](SECURITY.md) · [`THREAT_MODEL.md`](THREAT_MODEL.md) · [`ARCHITECTURE.md`](ARCHITECTURE.md).

## Building from source

```sh
cargo build --release --locked
cargo test --workspace --locked   # includes the security invariant tests
```

Requires Rust 1.85+. Windows is the primary target; macOS and Linux are built and tested in CI on a best-effort basis.

## Roadmap

Security-first, deliberately: **M0** trust boundary + CI (this) → **M1** Claude subscription provider, headless (`usage-cli --json`) → **M2** the always-on-top window → **M3** Codex provider → **M4** opt-in official billing APIs → **M5** history/forecasts → **M6** signed releases + packaging. See `ARCHITECTURE.md` §9.

## Disclaimer

QuotaPane is an independent, community project — **not affiliated with, endorsed, or supported by Anthropic or OpenAI.** The subscription/quota view relies on **undocumented endpoints that may change or break at any time**, uses **your own local credentials only**, bypasses no authentication, and scrapes nothing. To read subscription usage, QuotaPane sends the same `User-Agent: claude-code/<version>` header the official Claude Code client uses — the endpoint rate-limits requests without it — so these requests **present as the official client**; the Codex provider likewise sends the Codex CLI's default `User-Agent` (`codex-cli`). Each provider queries only the endpoint its official client already calls, with your own token, read-only. Use at your own risk.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. Contributions are accepted under the same terms.
