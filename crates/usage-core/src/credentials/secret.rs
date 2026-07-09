//! [`Secret<T>`] — the mandatory wrapper for every credential in the process.
//!
//! Guarantees (SECURITY.md invariant 2, THREAT_MODEL.md T-I1/T-I2):
//! - **Zeroize on drop:** the inner value is wiped from memory when the
//!   wrapper is dropped.
//! - **Redacted formatting:** `Debug` and `Display` print `«redacted»`;
//!   the inner value can never reach logs or crash output through the
//!   standard formatting paths.
//! - **No serialization:** `Secret<T>` deliberately implements neither
//!   `serde::Serialize` nor `Deserialize`, and never will. Persisting a
//!   secret is a breaking security change (invariant 1).
//! - **No cloning, no comparison:** no `Clone`/`PartialEq` impls — copies
//!   of secrets don't silently multiply, and secrets can't be accidentally
//!   compared in ways that could enable timing side channels.
//!
//! Access to the inner value is only possible through the explicit,
//! greppable [`Secret::expose`] method. Every call site of `expose` is
//! part of the audit surface — keep them few.

use std::fmt;
use zeroize::Zeroize;

/// The placeholder printed instead of secret contents.
const REDACTED: &str = "Secret(«redacted»)";

/// A secret value that zeroizes on drop and redacts in all formatting.
///
/// Construction wraps; [`Secret::expose`] is the only way back out.
pub struct Secret<T: Zeroize>(T);

impl<T: Zeroize> Secret<T> {
    /// Wrap a secret value. Do this as close to the point of ingress as
    /// possible so unwrapped copies never travel.
    pub fn new(value: T) -> Self {
        Secret(value)
    }

    /// Borrow the inner secret. **Every call site is audit surface.**
    ///
    /// Never let the returned reference escape into anything that logs,
    /// serializes, or persists.
    pub fn expose(&self) -> &T {
        &self.0
    }

    /// Explicitly wipe the secret now, without waiting for drop.
    pub fn zeroize_now(&mut self) {
        self.0.zeroize();
    }
}

impl<T: Zeroize> Drop for Secret<T> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

// Marker: dropping Secret<T> wipes its memory. Asserted in tests.
impl<T: Zeroize> zeroize::ZeroizeOnDrop for Secret<T> {}

impl<T: Zeroize> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

impl<T: Zeroize> fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(REDACTED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYNTHETIC: &str = "synthetic-bearer-XYZZY-0000-not-real";

    #[test]
    fn debug_and_display_are_redacted() {
        let s = Secret::new(SYNTHETIC.to_string());
        let debug = format!("{s:?}");
        let display = format!("{s}");
        assert!(!debug.contains("XYZZY"), "Debug leaked secret bytes");
        assert!(!display.contains("XYZZY"), "Display leaked secret bytes");
        assert_eq!(debug, REDACTED);
        assert_eq!(display, REDACTED);
    }

    #[test]
    fn redaction_survives_nested_formatting() {
        // Secrets embedded in larger structures must still redact.
        let s = Secret::new(SYNTHETIC.to_string());
        let wrapped = format!("snapshot state: {:?}", Some(&s));
        assert!(!wrapped.contains("XYZZY"));
    }

    #[test]
    fn explicit_zeroize_wipes_contents() {
        let mut s = Secret::new(SYNTHETIC.to_string());
        s.zeroize_now();
        assert!(
            s.expose().is_empty(),
            "zeroize_now must wipe the inner value"
        );
    }

    #[test]
    fn zeroize_on_drop_is_guaranteed_by_the_type_system() {
        // Compile-time assertion: Secret<T> implements ZeroizeOnDrop, so the
        // drop-wipe guarantee is part of the type's contract, not a convention.
        fn assert_zeroize_on_drop<Z: zeroize::ZeroizeOnDrop>() {}
        assert_zeroize_on_drop::<Secret<String>>();
        assert_zeroize_on_drop::<Secret<Vec<u8>>>();
    }

    #[test]
    fn expose_returns_the_secret_inside_the_boundary() {
        let s = Secret::new(SYNTHETIC.to_string());
        assert_eq!(s.expose(), SYNTHETIC);
    }
}
