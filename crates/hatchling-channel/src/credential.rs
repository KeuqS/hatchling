//! Channel credentials: the broker lends a backend its account secret
//! for one call, and nothing else ever holds it.
//!
//! A [`Credential`] is a borrowed view of secret bytes the broker owns
//! (loaded from the vault, zeroized when the broker drops them). It is
//! deliberately thin: no `Clone`, no `Serialize`, no `Display`, and a
//! redacted `Debug`, so a backend can authenticate with it but cannot
//! format it into a log line or put it on the wire by accident. It lives
//! only for the duration of the backend call it is passed to.

/// Account secret bytes, lent to a channel backend for one call.
pub struct Credential<'a>(&'a [u8]);

impl<'a> Credential<'a> {
    /// Lends `bytes` as a credential.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }

    /// The secret bytes, for the backend's outbound authentication only
    /// (signing, a bearer header at the broker edge). Never copy them
    /// into a [`Message`](crate::Message) or an error.
    #[must_use]
    pub const fn expose(&self) -> &'a [u8] {
        self.0
    }
}

impl std::fmt::Debug for Credential<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Credential([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use super::Credential;

    #[test]
    fn a_credential_exposes_its_bytes_and_debugs_redacted() {
        let key = b"hunter2-key-bytes";
        let credential = Credential::new(key);
        assert_eq!(credential.expose(), key);
        let shown = format!("{credential:?}");
        assert_eq!(shown, "Credential([redacted])");
        assert!(!shown.contains("hunter2"));
    }
}
