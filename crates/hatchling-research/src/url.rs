//! A source's address: an `https` URL the egress proxy can fetch.
//!
//! The form is what the proxy accepts: `https://host[:port][/path]`,
//! with a lowercase host, no userinfo, no query, no fragment, and no
//! `//` in the path. Whether the host may be reached at all is the
//! egress allowlist's call, not this parser's.

/// The longest source URL, in bytes.
pub const MAX_URL_BYTES: usize = 2048;

/// A parsed source URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceUrl {
    url: String,
    destination: String,
    path: String,
}

impl SourceUrl {
    /// Parses `raw`, or `None` when it is not a fetchable source URL.
    ///
    /// The URL is kept in one canonical form: the port is shown only
    /// when it is not 443, and an empty path is `/`.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        if raw.len() > MAX_URL_BYTES {
            return None;
        }
        let rest = raw.strip_prefix("https://")?;
        // The path, when there is one, starts at the first `/`.
        let (authority, path) = match rest.find('/') {
            Some(at) => (rest.get(..at)?, rest.get(at..)?),
            None => (rest, "/"),
        };
        let (host, port) = match authority.split_once(':') {
            Some((host, port)) => (host, parse_port(port)?),
            None => (authority, 443),
        };
        if !is_host(host) || !is_path(path) {
            return None;
        }
        let shown_port = if port == 443 {
            String::new()
        } else {
            format!(":{port}")
        };
        Some(Self {
            url: format!("https://{host}{shown_port}{path}"),
            destination: format!("{host}:{port}"),
            path: path.to_owned(),
        })
    }

    /// The URL, in canonical form.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.url
    }

    /// The egress destination, `host:port`.
    #[must_use]
    pub fn destination(&self) -> &str {
        &self.destination
    }

    /// The request path, starting with `/`.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}

/// A port: decimal digits only, 1 to 65535.
fn parse_port(raw: &str) -> Option<u16> {
    if !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    raw.parse().ok().filter(|port| *port != 0)
}

/// Dot-separated labels of lowercase letters, digits, and `-`.
fn is_host(host: &str) -> bool {
    host.split('.').all(|label| {
        !label.is_empty()
            && label
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    })
}

/// Printable ASCII, without `//`, a query, a fragment, or characters
/// that could leave a quoted label.
fn is_path(path: &str) -> bool {
    !path.contains("//")
        && path.bytes().all(|byte| {
            byte.is_ascii_graphic() && !matches!(byte, b'?' | b'#' | b'"' | b'<' | b'>' | b'\\')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(raw: &str) -> Option<(String, String, String)> {
        SourceUrl::parse(raw).map(|url| {
            (
                url.as_str().to_owned(),
                url.destination().to_owned(),
                url.path().to_owned(),
            )
        })
    }

    #[test]
    fn a_source_url_parses_to_one_canonical_form() {
        let own = |a: &str, b: &str, c: &str| Some((a.to_owned(), b.to_owned(), c.to_owned()));
        assert_eq!(
            parts("https://example.org"),
            own("https://example.org/", "example.org:443", "/")
        );
        assert_eq!(
            parts("https://example.org:443/a/b"),
            own("https://example.org/a/b", "example.org:443", "/a/b")
        );
        assert_eq!(
            parts("https://docs.example-1.org:8443/x%20y.html"),
            own(
                "https://docs.example-1.org:8443/x%20y.html",
                "docs.example-1.org:8443",
                "/x%20y.html"
            )
        );
    }

    #[test]
    fn anything_the_proxy_could_not_fetch_is_refused() {
        for bad in [
            "http://example.org/",
            "https://",
            "https:///a",
            "https://Example.org/",
            "https://ex ample.org/",
            "https://user@example.org/",
            "https://a..b/",
            "https://example.org./",
            "https://example.org:/",
            "https://example.org:0/",
            "https://example.org:+1/",
            "https://example.org:99999/",
            "https://example.org/a?b=1",
            "https://example.org/a#top",
            "https://example.org//a",
            "https://example.org/a b",
            "https://example.org/\"x",
            "https://example.org/<x>",
            "https://example.org/a\\b",
            "https://example.org/\u{e9}",
        ] {
            assert_eq!(SourceUrl::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_url_may_be_as_long_as_the_bound_and_no_longer() {
        let base = "https://example.org/";
        let at = format!("{base}{}", "a".repeat(MAX_URL_BYTES - base.len()));
        assert!(SourceUrl::parse(&at).is_some());
        let over = format!("{at}a");
        assert_eq!(SourceUrl::parse(&over), None);
        assert_eq!(MAX_URL_BYTES, 2048);
    }
}
