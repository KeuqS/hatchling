//! Secrets never cross the provider boundary.
//!
//! A model endpoint is a network peer that sees every request, and
//! "local" is not "trusted": a model on the user's desk sees the same
//! prompts a datacenter would. So every secret the agent has fetched
//! this session is registered here, and the provider replaces it in
//! every outbound request before the bytes reach the transport —
//! whatever the endpoint's locality. Locality changes the network
//! threat, not the data-minimization rule.
//!
//! A secret is replaced both as itself and in its JSON-escaped form,
//! since tool output reaches the model rendered as JSON text.

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde_json::Value;
use zeroize::Zeroizing;

/// What a secret reads as once redacted.
pub const REDACTED: &str = "[redacted secret]";

/// The session's registered secrets, shared between whatever fetches
/// them and the provider that must never send them.
///
/// [`Redactor::default`] is an empty registry. Cloning shares it. `Debug` shows only how many forms are
/// registered, never a secret.
#[derive(Clone, Default)]
pub struct Redactor {
    /// Every registered form, longest first, zeroized on drop.
    secrets: Arc<Mutex<Vec<Zeroizing<String>>>>,
}

impl fmt::Debug for Redactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Redactor")
            .field("forms", &self.lock().len())
            .finish()
    }
}

impl Redactor {
    /// Registers `secret`, as itself and as JSON escapes it. An empty
    /// secret registers nothing: it would match everywhere.
    pub fn register(&self, secret: &str) {
        if secret.is_empty() {
            return;
        }
        let mut forms = vec![secret.to_owned()];
        if let Some(escaped) = serde_json::to_string(secret).ok().and_then(|quoted| {
            quoted
                .strip_prefix('"')
                .and_then(|inner| inner.strip_suffix('"'))
                .map(str::to_owned)
        }) {
            if escaped != secret {
                forms.push(escaped);
            }
        }
        let mut secrets = self.lock();
        for form in forms {
            if !secrets.iter().any(|known| known.as_str() == form) {
                secrets.push(Zeroizing::new(form));
            }
        }
        // Longest first, so a secret that contains another is replaced
        // whole rather than leaving its remainder behind.
        secrets.sort_by_key(|known| std::cmp::Reverse(known.len()));
    }

    /// `text` with every registered secret replaced by [`REDACTED`].
    #[must_use]
    pub fn redact(&self, text: &str) -> String {
        self.lock().iter().fold(text.to_owned(), |out, secret| {
            out.replace(secret.as_str(), REDACTED)
        })
    }

    /// Redacts every string in `value`, however deeply nested.
    pub fn redact_value(&self, value: &mut Value) {
        match value {
            Value::String(text) => *text = self.redact(text),
            Value::Array(items) => items.iter_mut().for_each(|item| self.redact_value(item)),
            Value::Object(map) => map.values_mut().for_each(|item| self.redact_value(item)),
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }

    /// The registry. A panic elsewhere while it was held leaves the
    /// list itself intact, so a poisoned lock is still read: failing
    /// open to "no secrets known" is exactly what must not happen.
    fn lock(&self) -> MutexGuard<'_, Vec<Zeroizing<String>>> {
        self.secrets.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_registered_secret_is_replaced_wherever_it_appears() {
        let redactor = Redactor::default();
        assert_eq!(redactor.redact("the key is s3cr3t"), "the key is s3cr3t");
        redactor.register("s3cr3t");
        assert_eq!(
            redactor.redact("the key is s3cr3t, again s3cr3t"),
            "the key is [redacted secret], again [redacted secret]"
        );
        assert_eq!(REDACTED, "[redacted secret]");
        // Clones share the registry.
        let register_through = |shared: Redactor| shared.register("other");
        register_through(redactor.clone());
        assert_eq!(redactor.redact("other"), REDACTED);
    }

    #[test]
    fn a_secret_is_replaced_in_its_json_escaped_form_too() {
        let redactor = Redactor::default();
        redactor.register("a\"b\\c");
        let rendered = json!({"secret": "a\"b\\c"}).to_string();
        assert_eq!(rendered, r#"{"secret":"a\"b\\c"}"#);
        assert_eq!(
            redactor.redact(&rendered),
            r#"{"secret":"[redacted secret]"}"#
        );
        assert_eq!(redactor.redact("raw a\"b\\c"), "raw [redacted secret]");
        assert_eq!(format!("{redactor:?}"), "Redactor { forms: 2 }");
    }

    #[test]
    fn the_longest_secret_wins_and_nothing_is_registered_twice() {
        let redactor = Redactor::default();
        redactor.register("abc");
        redactor.register("abcdef");
        redactor.register("abc");
        assert_eq!(
            redactor.redact("abcdef abc"),
            "[redacted secret] [redacted secret]"
        );
        assert_eq!(format!("{redactor:?}"), "Redactor { forms: 2 }");
    }

    #[test]
    fn an_empty_secret_registers_nothing() {
        let redactor = Redactor::default();
        redactor.register("");
        assert_eq!(redactor.redact("abc"), "abc");
        assert_eq!(format!("{redactor:?}"), "Redactor { forms: 0 }");
    }

    #[test]
    fn every_string_in_a_value_is_redacted_however_deep() {
        let redactor = Redactor::default();
        redactor.register("s3cr3t");
        let mut value = json!({
            "messages": [
                {"role": "user", "content": "use s3cr3t"},
                {"role": "tool", "content": {"nested": ["x s3cr3t y", 1, true, null]}},
            ],
            "n": 7,
        });
        redactor.redact_value(&mut value);
        assert_eq!(
            value,
            json!({
                "messages": [
                    {"role": "user", "content": "use [redacted secret]"},
                    {"role": "tool", "content": {"nested": ["x [redacted secret] y", 1, true, null]}},
                ],
                "n": 7,
            })
        );
    }

    #[test]
    fn a_poisoned_registry_still_redacts() {
        let redactor = Redactor::default();
        redactor.register("s3cr3t");
        let shared = redactor.clone();
        let _ = std::thread::spawn(move || {
            let _guard = shared.secrets.lock();
            let _ = "not a number".parse::<u8>().expect("poison the registry");
        })
        .join();
        assert!(redactor.secrets.is_poisoned());
        assert_eq!(redactor.redact("s3cr3t"), REDACTED);
    }
}
