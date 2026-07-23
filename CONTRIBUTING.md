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
| `ureq` (+ `rustls` tree) | `usage-core::egress` only | The single HTTP client behind the egress chokepoint (invariant 3). Chosen over reqwest/tokio for a far smaller audit tree: synchronous, pure-Rust TLS, no async runtime. Configured with redirects disabled and proxy hard-off unless the user opts in. Pulls `rustls`, `ring`, `rustls-webpki`, `webpki-roots` (Mozilla root store) transitively — the price of doing TLS at all, and the smallest mainstream way to pay it. Notable transitives audited: `utf8-zero` (fork of Simon Sapin's `utf-8` maintained by ureq's own author, algesten; zero deps, no build script, no I/O — verified, not a typosquat); `log` facade (ureq redacts headers behind an allowlist and no binary here installs a backend; backends are banned in `deny.toml`). Re-verify both on ureq upgrades. | std has no HTTP or TLS. Hand-rolling either inside a trust boundary would be strictly worse. |
| `eframe` (egui, glow, winit tree) | `usage-ui` only | The M2 window: always-on-top frameless rendering, one memory-safe language, no webview/JS (ADR-001). Features trimmed: glow renderer only (no wgpu), default fonts, X11+Wayland. This is by far the largest subtree in the workspace, and it is deliberately confined to the render crate — `usage-core` (the trust boundary) has no path to any of it, and the UI receives only non-secret `ProviderSnapshot`s. Licenses added: Zlib, BSL-1.0, OFL-1.1, Ubuntu-font-1.0 (see deny.toml). Two build-time-only quick-xml advisories are ignored with written justification (deny.toml + .cargo/audit.toml). | A GUI cannot be hand-rolled; egui/eframe is the smallest mainstream pure-Rust option consistent with ADR-001. |
| `serde` (+ `derive`) / `serde_json` | `usage-core::credentials`, `usage-core::providers` | Parse `~/.claude/.credentials.json` and the usage response. `derive` is used for plain, non-secret DTOs only; the OAuth token is moved into `Secret<T>` immediately after deserialization and `Secret<T>` never derives `Serialize`/`Deserialize` (invariant 1). Pulls `syn`/`quote`/`proc-macro2` at build time (proc-macro, not in the runtime binary) and `itoa`/`ryu`/`memchr` at runtime. | Hand-writing a JSON parser inside the trust boundary would be more code and more risk than the ecosystem-standard, heavily-audited option. |
| `tray-icon` (+ `muda`) | `usage-ui` only, **Windows/macOS targets only** | The M3.5 system tray (icon, tooltip, Show/Hide/Quit menu). tauri-apps crate, actively maintained, backs the Tauri ecosystem. Top-tier reviewed 2026-07-20, corrected 2026-07-23 (`prompts/m3.5-tray-dependency-review.md`). Adding it puts **nine** entries in `Cargo.lock`: five that actually compile on Windows/macOS — `tray-icon`, `muda` (non-optional menu lib, same org), `crossbeam-channel` (≥0.5.16, post-RUSTSEC-2025-0024), `crossbeam-utils`, `keyboard-types` — plus **four phantom** crates pulled in only through `tray-icon`'s Linux/BSD-gated `dirs` dependency: `dirs`, `dirs-sys`, `redox_users` (all MIT/Apache), and `option-ext` (**MPL-2.0**). Because we gate `tray-icon` to `cfg(any(target_os="windows", target_os="macos"))` and `dirs` is Linux/BSD-only, that whole chain compiles on **no** target we build (verified: `cargo tree -i dirs --target <each>` is empty; it exists only as a `--target all` lockfile union artifact). All shipped crates are MIT/Apache with no advisories and no network capability; `dirs`/`dirs-sys` are filesystem-path helpers but are phantom (never compiled). The one non-permissive license, `option-ext`'s MPL-2.0, is handled by a narrow per-crate `deny.toml` exception (documented there) rather than by loosening the global allowlist or dropping Linux from the cargo-deny scan. Built with `default-features = false`: the `gtk` feature is excluded (the Linux backend needs the officially-unmaintained gtk-rs 0.18 + libappindicator), so **Linux is window-only in v1** (future gtk-free option: `ksni`, own review — which would make the phantom chain ship and require revisiting the exception). The tray receives only non-secret `ProviderSnapshot` data; `usage-core` gains nothing. The icon is generated as RGBA at runtime — no asset files, no build scripts, no png decode path. | A system tray cannot be reached from std; hand-rolling `Shell_NotifyIcon` FFI would put more unsafe code in this repo than the audited delta. |

(That's the whole tree today. Keep it that way where possible. Introducing a second HTTP client anywhere in the workspace is a breaking security change.)

## Development

```sh
cargo test --workspace --locked   # all tests, including invariant tests
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```

CI runs the same, plus `cargo-deny`, `cargo-audit`, and a no-telemetry check, on Windows/macOS/Linux.

## Reporting security issues

**Not** via public issues — see `SECURITY.md` for the private disclosure channel and our response commitments.
