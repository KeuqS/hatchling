//! Zeroizing secret bytes.
//!
//! [`Secret`] holds credential material. It zeroizes on drop (via
//! `zeroize`), has no `Display`, and its `Debug` is redacted: there is
//! no code path that formats a value into logs, errors, or panics.
//! Deliberately not `Clone` — copying secret bytes around multiplies
//! the memory that must be scrubbed.

use zeroize::Zeroize;

use crate::Error;
use hatchling_core::MAX_SECRET_BYTES;

/// Credential bytes. Zeroized on drop; never logged.
pub struct Secret(Vec<u8>);

impl Secret {
    /// Wraps `bytes`, rejecting oversize values before they are stored.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TooLarge`] if `bytes` exceeds
    /// [`MAX_SECRET_BYTES`].
    pub fn new(bytes: Vec<u8>) -> Result<Self, Error> {
        if bytes.len() > MAX_SECRET_BYTES {
            return Err(Error::TooLarge {
                len: bytes.len(),
                max: MAX_SECRET_BYTES,
            });
        }
        Ok(Self(bytes))
    }

    /// The secret bytes. The caller must not copy them further than
    /// necessary; the buffer is scrubbed when this `Secret` drops.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test-only allocator that never frees. With the real allocator,
    /// the `Vec`'s destructor frees the buffer immediately after
    /// `Secret::drop` zeroizes it, and the freed chunk is reused for
    /// freelist pointers — so a zeroized buffer can never be observed
    /// after a real `drop`. Here `dealloc` is a no-op, so the buffer
    /// survives and the test can read what `Drop` left behind.
    struct NeverFree;
    #[allow(
        unsafe_code,
        reason = "test-only GlobalAlloc impl; `alloc` delegates to System, `dealloc` is intentionally a no-op so dropped buffers stay observable"
    )]
    // SAFETY: `alloc`/`realloc` delegate to the system allocator with
    // the caller's layout; `dealloc` never frees, which is always
    // sound (leaking). No allocator invariants are violated.
    unsafe impl std::alloc::GlobalAlloc for NeverFree {
        unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
            // SAFETY: direct delegation to the system allocator.
            unsafe { std::alloc::System.alloc(layout) }
        }
        unsafe fn dealloc(&self, _ptr: *mut u8, _layout: std::alloc::Layout) {
            // Intentionally leak: the buffer must stay mapped.
        }
    }
    #[global_allocator]
    static NEVER_FREE: NeverFree = NeverFree;

    #[test]
    fn debug_never_contains_the_value() {
        // The whole point of the redacted Debug: deleting the custom
        // impl (falling back to a derived one) must fail this test.
        let secret = Secret::new(b"super-secret-value".to_vec()).unwrap();
        let rendered = format!("{secret:?}");
        assert!(!rendered.contains("super-secret-value"), "leak: {rendered}");
        assert_eq!(rendered, "Secret([redacted])");
    }

    #[test]
    fn rejects_oversize_at_construction() {
        // Boundary: exactly MAX_SECRET_BYTES is accepted, one more is
        // not. Pins the `>` against a `>=` mutant.
        let ok = vec![0u8; MAX_SECRET_BYTES];
        assert!(Secret::new(ok).is_ok());
        let big = vec![0u8; MAX_SECRET_BYTES + 1];
        let err = Secret::new(big).unwrap_err();
        assert!(
            matches!(err, Error::TooLarge { len, max } if len == MAX_SECRET_BYTES + 1 && max == MAX_SECRET_BYTES)
        );
    }

    #[test]
    fn exposes_the_bytes() {
        let secret = Secret::new(b"abc".to_vec()).unwrap();
        assert_eq!(secret.as_bytes(), b"abc");
    }

    #[test]
    #[allow(
        unsafe_code,
        reason = "reads the dropped buffer through a raw pointer; NeverFree keeps it mapped, and each read carries a SAFETY comment"
    )]
    fn drop_zeroizes_the_buffer() {
        // The whole point of the `Drop` impl: the `drop -> ()` mutant
        // (deleting it) must fail this test. The `NeverFree` allocator
        // above keeps the buffer mapped after `drop`, so the
        // zeroization is observable.
        let secret = Secret::new(vec![0xABu8; 64]).unwrap();
        let (ptr, len) = (secret.as_bytes().as_ptr(), secret.as_bytes().len());
        // Precondition: the buffer really holds the secret, so the
        // final assertion cannot pass vacuously.
        let before = {
            // SAFETY: `ptr`/`len` describe the live `Secret`'s buffer.
            unsafe { std::slice::from_raw_parts(ptr, len) }
        };
        assert!(before.iter().all(|&b| b == 0xAB));
        drop(secret);
        // SAFETY: `NeverFree` never unmaps; the only write since the
        // precondition was `Drop`'s zeroization.
        let after = unsafe { std::slice::from_raw_parts(ptr, len) };
        assert!(
            after.iter().all(|&b| b == 0),
            "dropped Secret left secret bytes in memory"
        );
    }
}
