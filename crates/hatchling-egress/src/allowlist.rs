//! Destination allowlist: deny-by-default egress control.
//!
//! The allowlist is parsed once at startup from
//! `HATCHLING_EGRESS_ALLOWLIST` (comma-separated `host:port`). Every
//! entry must be explicit — no wildcards, no IP literals, no CIDR
//! ranges. An invalid entry fails the proxy closed at startup; there
//! is no "allow the valid ones and skip the bad" — a misconfigured
//! allowlist is a fail-closed proxy.
//!
//! At request time, the destination is checked against the allowlist
//! with exact string matching. DNS is resolved once at startup and the
//! IPs are pinned, eliminating DNS-rebinding between the allowlist
//! check and the connection.
//!
//! **Public sources.** An entry written `public:host:port` is
//! a public source, such as a page a research run reads. It is allowed
//! like any other destination, but the proxy only ever reads from it
//! (`GET`, no body) and never attaches a credential to it. A
//! destination is one kind or the other: listing it both ways is
//! ambiguous, and fails the allowlist closed.
//!
//! **Local model endpoints.** An entry written
//! `local:localhost:port` is a model server on this machine, such as
//! ollama or llama.cpp. "Local" is a claim to verify, not a trust
//! label: the host must be `localhost`, every address it resolves to
//! at startup must be `127.0.0.1` or `::1` (anything else, say from a
//! poisoned hosts file, fails the allowlist closed), and the proxy
//! connects only to those pinned addresses and checks each
//! connection's peer before sending a byte (see [`crate::local`]). A
//! local endpoint is spoken to in plain HTTP. Its credential is the
//! vault's, exactly as for any other destination; a server that needs
//! none is written `local-keyless:localhost:port`, so going without a
//! credential is always the operator's explicit choice, never what a
//! missing secret silently turns into.

use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};

use crate::Error;

/// The prefix that marks a public source in the allowlist.
pub const PUBLIC_PREFIX: &str = "public:";

/// The prefix that marks a local model endpoint in the allowlist
/// loopback-pinned, plain HTTP, with the vault's credential.
pub const LOCAL_PREFIX: &str = "local:";

/// The prefix that marks a local model endpoint that takes no
/// credential: the explicit keyless marker.
pub const LOCAL_KEYLESS_PREFIX: &str = "local-keyless:";

/// The one host a local endpoint may name.
pub const LOCAL_HOST: &str = "localhost";

/// What kind of destination an allowlist entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A credentialed HTTPS destination.
    Credentialed,
    /// A public source: read-only, never credentialed.
    Public,
    /// A local model endpoint, credentialed unless `keyless`.
    Local {
        /// The operator's explicit marker that it takes no credential.
        keyless: bool,
    },
}

impl Kind {
    /// Splits an entry's kind prefix from its destination.
    fn of(entry: &str) -> (Self, &str) {
        [
            (LOCAL_KEYLESS_PREFIX, Self::Local { keyless: true }),
            (LOCAL_PREFIX, Self::Local { keyless: false }),
            (PUBLIC_PREFIX, Self::Public),
        ]
        .into_iter()
        .find_map(|(prefix, kind)| entry.strip_prefix(prefix).map(|rest| (kind, rest)))
        .unwrap_or((Self::Credentialed, entry))
    }
}

/// A local model endpoint the allowlist admits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Local {
    /// The loopback addresses `localhost` resolved to at startup, IPv4
    /// first. The proxy connects to these and nothing else.
    addrs: Vec<SocketAddr>,
    /// Whether the operator marked it as taking no credential.
    keyless: bool,
}

impl Local {
    /// The pinned loopback addresses, IPv4 first.
    #[must_use]
    pub fn addrs(&self) -> &[SocketAddr] {
        &self.addrs
    }

    /// Whether the operator marked the endpoint as taking no credential.
    #[must_use]
    pub const fn keyless(&self) -> bool {
        self.keyless
    }
}

/// Whether `addr` is one of the two loopback addresses a local
/// endpoint may be pinned to: exactly `127.0.0.1` or `::1`.
#[must_use]
pub fn is_pinned_loopback(addr: &SocketAddr) -> bool {
    addr.ip() == IpAddr::V4(Ipv4Addr::LOCALHOST) || addr.ip() == IpAddr::V6(Ipv6Addr::LOCALHOST)
}

/// Pins a local endpoint's startup resolution: every address must be
/// loopback (a `localhost` that resolves anywhere else is an
/// impersonation, and fails closed), ordered IPv4 first.
fn pin_loopback(entry: &str, mut resolved: Vec<SocketAddr>) -> Result<Vec<SocketAddr>, Error> {
    if let Some(addr) = resolved.iter().find(|addr| !is_pinned_loopback(addr)) {
        return Err(Error::InvalidAllowlist {
            reason: format!(
                "local endpoint {entry:?} resolves to {}, which is not loopback",
                addr.ip()
            ),
        });
    }
    resolved.sort_by_key(SocketAddr::is_ipv6);
    Ok(resolved)
}

/// A parsed egress allowlist: the exact set of permitted destinations.
#[derive(Debug, Clone)]
pub struct Allowlist {
    /// The allowed `host:port` strings, for exact matching.
    destinations: HashSet<String>,
    /// The allowed destinations that are public sources: read-only,
    /// and never given a credential.
    public: HashSet<String>,
    /// The allowed destinations that are local model endpoints, with
    /// their pinned loopback addresses.
    local: HashMap<String, Local>,
    /// Each destination's socket addresses, resolved once at startup.
    /// The proxy connects to these, never to a fresh DNS lookup.
    pinned: HashMap<String, Vec<SocketAddr>>,
}

impl Allowlist {
    /// Parses an allowlist from a comma-separated `host:port` string.
    ///
    /// Every entry must be a valid destination (see
    /// `surrogate::is_valid_destination`): explicit hostname, numeric
    /// port, no wildcards, no IP literals. Empty entries are ignored.
    ///
    /// An empty allowlist is valid: it denies everything. This is the
    /// fail-closed default — the broker serves no egress until the
    /// operator configures destinations.
    ///
    /// DNS is resolved now and the addresses pinned. A hostname that
    /// does not resolve is an error — fail closed, don't defer the
    /// failure to request time.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidAllowlist`] if any entry is malformed
    /// or if a hostname fails to resolve.
    pub fn parse(raw: &str) -> Result<Self, Error> {
        let mut destinations = HashSet::new();
        let mut public = HashSet::new();
        let mut local = HashMap::new();
        let mut kinds: HashMap<String, Kind> = HashMap::new();
        let mut pinned = HashMap::new();

        for entry in raw.split(',') {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            let (kind, entry) = Kind::of(entry);
            // Reuse the surrogate destination validator: the same
            // rules apply (explicit host:port, no wildcards, no IP
            // literals).
            if !crate::surrogate::is_valid_destination(entry) {
                return Err(Error::InvalidAllowlist {
                    reason: format!("invalid destination {entry:?}"),
                });
            }
            if matches!(kind, Kind::Local { .. })
                && entry.rsplit_once(':').map(|(host, _)| host) != Some(LOCAL_HOST)
            {
                return Err(Error::InvalidAllowlist {
                    reason: format!("local endpoint {entry:?} must be {LOCAL_HOST}:port"),
                });
            }
            // Pin the DNS resolution now. If it fails, the allowlist
            // is misconfigured — fail closed at startup.
            let resolved: Vec<SocketAddr> = entry
                .to_socket_addrs()
                .map_err(|_| Error::InvalidAllowlist {
                    reason: format!("destination {entry:?} does not resolve"),
                })?
                .collect();
            if resolved.is_empty() {
                return Err(Error::InvalidAllowlist {
                    reason: format!("destination {entry:?} resolved to no addresses"),
                });
            }
            // One destination, one kind: a public source and a
            // credentialed API under the same name is ambiguous.
            if kinds.get(entry).is_some_and(|listed| *listed != kind) {
                return Err(Error::InvalidAllowlist {
                    reason: format!("destination {entry:?} is listed as two different kinds"),
                });
            }
            kinds.insert(entry.to_owned(), kind);
            destinations.insert(entry.to_owned());
            match kind {
                Kind::Public => {
                    public.insert(entry.to_owned());
                }
                Kind::Local { keyless } => {
                    let pinned = pin_loopback(entry, resolved.clone())?;
                    local.insert(
                        entry.to_owned(),
                        Local {
                            addrs: pinned,
                            keyless,
                        },
                    );
                }
                Kind::Credentialed => {}
            }
            pinned.insert(entry.to_owned(), resolved);
        }

        Ok(Self {
            destinations,
            public,
            local,
            pinned,
        })
    }

    /// The local model endpoint at `destination`, if the allowlist
    /// lists it as one.
    #[must_use]
    pub fn local(&self, destination: &str) -> Option<&Local> {
        self.local.get(destination)
    }

    /// Returns `true` if the destination is an allowed public source:
    /// read-only, and never given a credential.
    #[must_use]
    pub fn is_public(&self, destination: &str) -> bool {
        self.public.contains(destination)
    }

    /// Returns `true` if the destination is on the allowlist (exact
    /// string match).
    #[must_use]
    pub fn allows(&self, destination: &str) -> bool {
        self.destinations.contains(destination)
    }

    /// The socket addresses an allowed destination was pinned to at
    /// startup, or `None` if the destination is not on the allowlist. No
    /// lookup happens here: these are the only addresses the proxy will
    /// connect to for it.
    #[must_use]
    pub fn addrs_for(&self, destination: &str) -> Option<&[SocketAddr]> {
        self.pinned.get(destination).map(Vec::as_slice)
    }

    /// The number of allowed destinations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.destinations.len()
    }

    /// Returns `true` if the allowlist has no destinations. (Cannot
    /// happen via [`parse`](Self::parse), which rejects empty
    /// allowlists — this is for completeness.)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.destinations.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_allowlist() {
        // Use localhost, which always resolves.
        let allowlist = Allowlist::parse("localhost:443,localhost:80").unwrap();
        assert_eq!(allowlist.len(), 2);
        assert!(!allowlist.is_empty());
        assert!(allowlist.allows("localhost:443"));
        assert!(allowlist.allows("localhost:80"));
        assert!(!allowlist.allows("localhost:8080"));
        assert!(!allowlist.allows("example.com:443"));
    }

    #[test]
    fn rejects_wildcards() {
        assert!(Allowlist::parse("*.example.com:443").is_err());
    }

    #[test]
    fn rejects_ip_literals() {
        assert!(Allowlist::parse("192.168.1.1:443").is_err());
        assert!(Allowlist::parse("[::1]:443").is_err());
    }

    #[test]
    fn rejects_malformed_entries() {
        // Note: "" and "   " and ",,," are NOT in this list — an empty
        // allowlist is valid (deny everything). See
        // `ignores_empty_entries`.
        for bad in [
            "not-a-destination",
            "host:abc",
            "host:0",
            "https://example.com:443",
            "example.com:443/path",
        ] {
            assert!(Allowlist::parse(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn empty_allowlist_denies_everything() {
        // An empty allowlist is valid: it denies everything. This is
        // the fail-closed default.
        for empty in ["", "   ", ",,,"] {
            let allowlist = Allowlist::parse(empty).unwrap();
            assert!(allowlist.is_empty());
            assert!(!allowlist.allows("localhost:443"));
        }
    }

    #[test]
    fn ignores_empty_entries() {
        let allowlist = Allowlist::parse("localhost:443,, localhost:80 ,").unwrap();
        assert_eq!(allowlist.len(), 2);
    }

    #[test]
    fn public_sources_are_allowed_and_marked() {
        let allowlist = Allowlist::parse("public:localhost:443, localhost:80").unwrap();
        assert_eq!(allowlist.len(), 2);
        assert!(allowlist.allows("localhost:443"));
        assert!(allowlist.allows("localhost:80"));
        assert!(allowlist.is_public("localhost:443"));
        assert!(!allowlist.is_public("localhost:80"));
        assert!(!allowlist.is_public("example.com:443"));
        // The same kind twice is one destination.
        let twice = Allowlist::parse("public:localhost:443,public:localhost:443").unwrap();
        assert_eq!(twice.len(), 1);
        assert!(twice.is_public("localhost:443"));
        let plain = Allowlist::parse("localhost:443,localhost:443").unwrap();
        assert!(!plain.is_public("localhost:443"));
        assert_eq!(PUBLIC_PREFIX, "public:");
    }

    #[test]
    fn a_destination_listed_both_ways_or_a_bad_public_entry_fails_closed() {
        for bad in [
            "public:localhost:443,localhost:443",
            "localhost:443,public:localhost:443",
            "public:*.example.com:443",
            "public:192.168.1.1:443",
            "public:",
            "public: localhost:443",
        ] {
            assert!(Allowlist::parse(bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn a_local_endpoint_is_pinned_to_loopback_and_marked() {
        let allowlist =
            Allowlist::parse("local:localhost:11434, local-keyless:localhost:8080, localhost:443")
                .unwrap();
        assert_eq!(allowlist.len(), 3);
        assert!(allowlist.allows("localhost:11434"));
        assert!(allowlist.allows("localhost:8080"));
        let keyed = allowlist.local("localhost:11434").unwrap();
        assert!(!keyed.keyless());
        let keyless = allowlist.local("localhost:8080").unwrap();
        assert!(keyless.keyless());
        for local in [keyed, keyless] {
            assert!(!local.addrs().is_empty());
            assert!(local.addrs().iter().all(is_pinned_loopback));
        }
        assert!(keyed.addrs().iter().all(|addr| addr.port() == 11434));
        // Neither an ordinary destination nor a public source is local,
        // and a local endpoint is not a public source.
        assert!(allowlist.local("localhost:443").is_none());
        assert!(allowlist.local("example.com:443").is_none());
        assert!(!allowlist.is_public("localhost:11434"));
        assert_eq!(
            (LOCAL_PREFIX, LOCAL_KEYLESS_PREFIX, LOCAL_HOST),
            ("local:", "local-keyless:", "localhost")
        );
    }

    #[test]
    fn a_local_endpoint_must_name_localhost_and_be_one_kind() {
        for bad in [
            "local:example.com:11434",
            "local:localhost.example.com:11434",
            "local:127.0.0.1:11434",
            "local:[::1]:11434",
            "local-keyless:example.com:8080",
            "local:localhost",
            "local:",
            "local:localhost:11434,localhost:11434",
            "local:localhost:11434,local-keyless:localhost:11434",
            "public:localhost:11434,local:localhost:11434",
        ] {
            assert!(Allowlist::parse(bad).is_err(), "should reject {bad:?}");
        }
        // The same kind twice is one destination.
        let twice = Allowlist::parse("local:localhost:11434,local:localhost:11434").unwrap();
        assert_eq!(twice.len(), 1);
    }

    #[test]
    fn only_the_two_loopback_addresses_are_pinnable() {
        let addr = |text: &str| text.parse::<SocketAddr>().unwrap();
        assert!(is_pinned_loopback(&addr("127.0.0.1:11434")));
        assert!(is_pinned_loopback(&addr("[::1]:11434")));
        for other in [
            "127.0.0.2:11434",
            "10.0.0.5:11434",
            "0.0.0.0:11434",
            "[::]:11434",
            "[::ffff:127.0.0.1]:11434",
            "93.184.216.34:11434",
        ] {
            assert!(!is_pinned_loopback(&addr(other)), "{other} is not pinnable");
        }
    }

    #[test]
    fn a_localhost_that_resolves_off_the_machine_fails_closed() {
        // A poisoned hosts file or a rebinding resolver: one address of
        // the resolution is remote, and the entry is refused outright.
        let addr = |text: &str| text.parse::<SocketAddr>().unwrap();
        let poisoned = vec![addr("127.0.0.1:11434"), addr("10.0.0.5:11434")];
        let error = pin_loopback("localhost:11434", poisoned).unwrap_err();
        assert!(
            matches!(&error, Error::InvalidAllowlist { reason } if reason == "local endpoint \"localhost:11434\" resolves to 10.0.0.5, which is not loopback"),
            "got {error:?}"
        );
        // A clean resolution is kept, IPv4 first.
        let pinned = pin_loopback(
            "localhost:11434",
            vec![addr("[::1]:11434"), addr("127.0.0.1:11434")],
        )
        .unwrap();
        assert_eq!(pinned, vec![addr("127.0.0.1:11434"), addr("[::1]:11434")]);
    }

    #[test]
    fn addrs_for_returns_pinned_addresses() {
        let allowlist = Allowlist::parse("localhost:443").unwrap();
        let addrs = allowlist.addrs_for("localhost:443").unwrap();
        assert!(!addrs.is_empty());
        assert!(
            addrs
                .iter()
                .all(|addr| addr.port() == 443 && addr.ip().is_loopback())
        );
        assert!(allowlist.addrs_for("localhost:80").is_none());
        assert!(allowlist.addrs_for("example.com:443").is_none());
    }
}
