//! Strict endpoint parsing.
//!
//! The provider only needs scheme/authority/path splitting, so this
//! is hand-rolled rather than pulling in a URL crate. The rules are
//! deliberately narrow — anything unusual fails closed at
//! construction, not deep in a request:
//!
//! - Scheme must be `https`, except for a **local endpoint**:
//!   `http://localhost[:port]`, a model server on this machine, which
//!   the broker reaches in plain HTTP over a loopback connection it
//!   pins and verifies. Plain `http` to any other host is refused: the
//!   egress path to anywhere else is HTTPS-only. A local endpoint is
//!   written with `localhost`, not an IP literal, so that the
//!   allowlist entry naming it is the one the broker pins.
//! - No userinfo (`@`), no query (`?`), no fragment (`#`).
//! - Authority is `host` or `host:port`; bracketed IPv6 (`[::1]`)
//!   with an optional port is accepted. The port defaults to 443
//!   (`https`) or 80 (`http`).
//! - The path prefix is empty or starts with `/`; it must not
//!   contain whitespace or control characters.

use crate::Error;

/// The one host a local endpoint may name.
pub const LOCAL_HOST: &str = "localhost";

/// A validated model endpoint, split the way the [`Transport`][crate::Transport]
/// wants it: `destination` (`host:port`) plus a path prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    destination: String,
    path_prefix: String,
    local: bool,
}

impl Endpoint {
    /// Parse and validate an endpoint URL.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidEndpoint`] when the input is not a
    /// well-formed `https://` URL, or `http://localhost` URL, under the
    /// rules above.
    pub fn parse(input: &str) -> Result<Self, Error> {
        let (local, authority_and_path) = if let Some(rest) = input.strip_prefix("https://") {
            (false, rest)
        } else if let Some(rest) = input.strip_prefix("http://") {
            (true, rest)
        } else {
            return Err(Error::InvalidEndpoint {
                reason: String::from(
                    "endpoint must use the https scheme, or http for a local endpoint at localhost",
                ),
            });
        };
        if authority_and_path.is_empty() {
            return Err(Error::InvalidEndpoint {
                reason: String::from("endpoint has no authority"),
            });
        }
        let path_start = authority_and_path
            .find('/')
            .unwrap_or(authority_and_path.len());
        let (authority, path) = authority_and_path.split_at(path_start);
        reject_bad_characters(authority, "authority")?;
        reject_bad_characters(path, "path")?;
        if path.contains(['?', '#']) {
            return Err(Error::InvalidEndpoint {
                reason: String::from("endpoint must not contain a query or fragment"),
            });
        }
        let destination = parse_authority(authority, if local { 80 } else { 443 })?;
        if local && destination.rsplit_once(':').map(|(host, _)| host) != Some(LOCAL_HOST) {
            return Err(Error::InvalidEndpoint {
                reason: format!(
                    "plain http is only for a local endpoint, written http://{LOCAL_HOST}:port"
                ),
            });
        }
        let path_prefix = path.trim_end_matches('/').to_owned();
        Ok(Self {
            destination,
            path_prefix,
            local,
        })
    }

    /// Whether this is a local endpoint (`http://localhost`), which the
    /// broker reaches over a pinned, verified loopback connection.
    #[must_use]
    pub const fn is_local(&self) -> bool {
        self.local
    }

    /// `host:port` for the transport.
    #[must_use]
    pub fn destination(&self) -> &str {
        &self.destination
    }

    /// Full request path for chat completions, e.g.
    /// `/v1/chat/completions`.
    #[must_use]
    pub fn chat_completions_path(&self) -> String {
        format!("{}/chat/completions", self.path_prefix)
    }
}

/// Rejects userinfo markers, whitespace, and control characters —
/// none of them have business in an endpoint.
fn reject_bad_characters(part: &str, what: &str) -> Result<(), Error> {
    if part.contains('@') || part.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(Error::InvalidEndpoint {
            reason: format!("endpoint {what} contains a forbidden character"),
        });
    }
    Ok(())
}

/// Splits `host` / `host:port` / `[v6]` / `[v6]:port` into the
/// transport's `host:port` form, `default_port` filling an absent port.
fn parse_authority(authority: &str, default_port: u16) -> Result<String, Error> {
    if authority.is_empty() {
        return Err(Error::InvalidEndpoint {
            reason: String::from("endpoint authority is empty"),
        });
    }
    if let Some(after_bracket) = authority.strip_prefix('[') {
        let (host, rest) = after_bracket
            .split_once(']')
            .ok_or_else(|| Error::InvalidEndpoint {
                reason: String::from("endpoint has an unterminated IPv6 bracket"),
            })?;
        if host.is_empty() {
            return Err(Error::InvalidEndpoint {
                reason: String::from("endpoint IPv6 host is empty"),
            });
        }
        let port = parse_port_suffix(rest, default_port)?;
        return Ok(format!("[{host}]:{port}"));
    }
    match authority.rsplit_once(':') {
        Some((host, port_raw)) => {
            if host.is_empty() {
                return Err(Error::InvalidEndpoint {
                    reason: String::from("endpoint host is empty"),
                });
            }
            let port = parse_port(port_raw)?;
            Ok(format!("{host}:{port}"))
        }
        None => Ok(format!("{authority}:{default_port}")),
    }
}

/// Parses the `:port` suffix after a bracketed IPv6 host: empty means
/// the default, otherwise a valid port number.
fn parse_port_suffix(rest: &str, default_port: u16) -> Result<u16, Error> {
    if rest.is_empty() {
        return Ok(default_port);
    }
    let port_raw = rest
        .strip_prefix(':')
        .ok_or_else(|| Error::InvalidEndpoint {
            reason: String::from("unexpected characters after IPv6 bracket"),
        })?;
    parse_port(port_raw)
}

fn parse_port(raw: &str) -> Result<u16, Error> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::InvalidEndpoint {
            reason: String::from("endpoint port must be a base-10 number"),
        });
    }
    raw.parse::<u16>().map_err(|_| Error::InvalidEndpoint {
        reason: String::from("endpoint port out of range"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bare_host_defaults_port_and_path() {
        let endpoint = Endpoint::parse("https://models.example.com").unwrap();
        assert_eq!(endpoint.destination(), "models.example.com:443");
        assert_eq!(endpoint.chat_completions_path(), "/chat/completions");
    }

    #[test]
    fn parse_host_port_and_prefix() {
        let endpoint = Endpoint::parse("https://models.example.com:8443/v1/").unwrap();
        assert_eq!(endpoint.destination(), "models.example.com:8443");
        assert_eq!(endpoint.chat_completions_path(), "/v1/chat/completions");
    }

    #[test]
    fn parse_ipv6_with_and_without_port() {
        let with_port = Endpoint::parse("https://[fd00::1]:8443/v1").unwrap();
        assert_eq!(with_port.destination(), "[fd00::1]:8443");
        let bare = Endpoint::parse("https://[fd00::1]").unwrap();
        assert_eq!(bare.destination(), "[fd00::1]:443");
    }

    #[test]
    fn reject_non_https_schemes() {
        for input in [
            "http://models.example.com",
            "ftp://models.example.com",
            "models.example.com",
            "",
        ] {
            assert!(Endpoint::parse(input).is_err(), "should reject {input:?}");
        }
    }

    #[test]
    fn reject_malformed_authorities() {
        for input in [
            "https://",
            "https:///v1",
            "https://:8443",
            "https://user@models.example.com",
            "https://models.example.com:notaport",
            "https://models.example.com:99999",
            "https://models.example.com:",
            "https://[fd00::1",
            "https://[]/v1",
            "https://[fd00::1]x/v1",
            "https://mod els.example.com",
        ] {
            assert!(Endpoint::parse(input).is_err(), "should reject {input:?}");
        }
    }

    #[test]
    fn reject_query_fragment_and_bad_path() {
        for input in [
            "https://models.example.com/v1?key=abc",
            "https://models.example.com/v1#frag",
            "https://models.example.com/v 1",
        ] {
            assert!(Endpoint::parse(input).is_err(), "should reject {input:?}");
        }
    }

    #[test]
    fn a_local_endpoint_is_http_at_localhost() {
        let endpoint = Endpoint::parse("http://localhost:11434/v1").unwrap();
        assert_eq!(endpoint.destination(), "localhost:11434");
        assert_eq!(endpoint.chat_completions_path(), "/v1/chat/completions");
        assert!(endpoint.is_local());
        assert_eq!(
            Endpoint::parse("http://localhost").unwrap().destination(),
            "localhost:80"
        );
        // https at localhost is an ordinary endpoint, not a local one.
        let tls = Endpoint::parse("https://localhost:8443/v1").unwrap();
        assert_eq!(tls.destination(), "localhost:8443");
        assert!(!tls.is_local());
        assert!(
            !Endpoint::parse("https://models.example.com")
                .unwrap()
                .is_local()
        );
        assert_eq!(LOCAL_HOST, "localhost");
    }

    #[test]
    fn plain_http_anywhere_but_localhost_is_refused() {
        for input in [
            "http://models.example.com/v1",
            "http://127.0.0.1:11434/v1",
            "http://[::1]:11434/v1",
            "http://localhost.example.com:11434",
            "http://evil.example.com/localhost:11434",
            "http://",
            "http://user@localhost:11434",
            "http://localhost:11434/v1?x=1",
        ] {
            let error = Endpoint::parse(input).unwrap_err();
            assert!(matches!(error, Error::InvalidEndpoint { .. }), "{input}");
        }
        let error = Endpoint::parse("http://127.0.0.1:11434").unwrap_err();
        assert!(
            matches!(&error, Error::InvalidEndpoint { reason } if reason == "plain http is only for a local endpoint, written http://localhost:port"),
            "got {error:?}"
        );
        let error = Endpoint::parse("ftp://localhost").unwrap_err();
        assert!(
            matches!(&error, Error::InvalidEndpoint { reason } if reason == "endpoint must use the https scheme, or http for a local endpoint at localhost"),
            "got {error:?}"
        );
    }

    #[test]
    fn empty_port_rejected_as_not_a_number() {
        // Pins the `||` in `parse_port`: an empty port must fail the
        // not-a-number check, not fall through to the range check.
        // (`||` -> `&&` still errors, but with the wrong reason.)
        let error = Endpoint::parse("https://models.example.com:").unwrap_err();
        assert!(
            matches!(&error, Error::InvalidEndpoint { reason } if reason.contains("base-10")),
            "empty port must be rejected as not-a-number, got {error:?}"
        );
    }
}
