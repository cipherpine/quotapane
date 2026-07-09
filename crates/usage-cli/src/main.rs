//! QuotaPane headless CLI — `--json` lands in M1 with the first provider.
//!
//! This binary is how the pipeline is proven before any UI exists, and how
//! security-conscious users verify egress behavior under a packet capture
//! (SECURITY.md, hardening guidance §3).

use usage_core::egress::ALLOWED_HOSTS;

fn main() {
    println!("quotapane usage-cli (M0 skeleton)");
    println!("Providers arrive in M1. Current egress allowlist (deny-by-default):");
    for host in ALLOWED_HOSTS {
        println!("  - {host}");
    }
    println!("Any other host is a hard error, enforced by usage-core::egress and its tests.");
}
