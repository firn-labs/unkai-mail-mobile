//! Small URL guards used by the protocol crates.
//!
//! These exist to make sure we never accidentally send credentials
//! over an unencrypted transport.  IMAP, SMTP, JMAP, CalDAV and
//! CardDAV all carry HTTP Basic auth (or comparable) headers in the
//! request itself; if the URL turned out to be `http://` instead
//! of `https://` (server misconfig, autoconfig fallback bug, a
//! freshly-typed account where the user dropped the `s`) we'd leak
//! the password in cleartext.  Calling `ensure_https()` before any
//! authenticated request short-circuits with a clear error instead
//! of trusting the URL.

use std::net::{Ipv4Addr, Ipv6Addr};

// `::url` is the WHATWG URL crate, not this module (which is also
// called `url`).
use ::url::{Host, Url};

use crate::UnkaiError;

/// Reject any URL that doesn't carry the `https://` scheme — with a
/// narrow exemption for HTTP loopback (`127.0.0.1`, `[::1]`,
/// `localhost`).
///
/// This is a defence-in-depth guard.  Account discovery and the
/// autoconfig flow already filter out HTTP endpoints, so under
/// normal use this never trips — but a misconfigured Nextcloud
/// instance, a typo in a manually-entered server URL, or a future
/// regression in discovery would otherwise silently put credentials
/// over plaintext HTTP.  Refuse to proceed instead.
///
/// The loopback exemption exists because:
///   * mock servers in `tests/` bind to `http://127.0.0.1:<port>`
///     and the integration tests pass real credentials to them.
///     Loopback never leaves the machine, so cleartext is safe.
///   * Developers running a local Nextcloud instance for testing
///     also commonly do so over plain HTTP on localhost.
///
/// ## Why this parses with the `url` crate
///
/// The guard must see the *same host the request will go to*, so it
/// parses with the WHATWG URL parser `reqwest` uses rather than
/// splitting the string by hand. The hand-rolled version this replaced
/// disagreed with it on a backslash: WHATWG reads `\` as `/` in an
/// `http` URL, so `http://evil.com\@localhost/` *connects to
/// `evil.com`*, while splitting on `/` and `@` saw the host
/// `localhost` and let the password go out in cleartext.
pub fn ensure_https(url: &str) -> Result<(), UnkaiError> {
    let allowed = match Url::parse(url) {
        Ok(parsed) => match parsed.scheme() {
            "https" => true,
            "http" => is_loopback(&parsed),
            _ => false,
        },
        Err(_) => false,
    };
    if allowed {
        Ok(())
    } else {
        Err(UnkaiError::Network(format!(
            "refusing to send credentials over a non-HTTPS URL: {url}"
        )))
    }
}

/// Exactly the three loopback spellings the exemption is for. The
/// parser has already lower-cased the host and decoded any numeric
/// IPv4 form, so `http://LOCALHOST/` and `http://0x7f.1/` land here as
/// `localhost` and `127.0.0.1`.
fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(name)) => name == "localhost",
        Some(Host::Ipv4(ip)) => ip == Ipv4Addr::LOCALHOST,
        Some(Host::Ipv6(ip)) => ip == Ipv6Addr::LOCALHOST,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_passes() {
        assert!(ensure_https("https://example.com/path").is_ok());
    }

    #[test]
    fn http_loopback_passes() {
        assert!(ensure_https("http://127.0.0.1:8080/path").is_ok());
        assert!(ensure_https("http://localhost:3000/").is_ok());
        assert!(ensure_https("http://[::1]:8080/api").is_ok());
    }

    #[test]
    fn http_public_rejected() {
        let err = ensure_https("http://example.com/path").unwrap_err();
        assert!(format!("{err}").contains("non-HTTPS"));
    }

    #[test]
    fn http_userinfo_does_not_spoof_loopback() {
        // `userinfo@host` — the userinfo is `127.0.0.1`, host is
        // `evil.com`.  Must reject.
        assert!(ensure_https("http://127.0.0.1@evil.com/path").is_err());
    }

    #[test]
    fn http_backslash_does_not_spoof_loopback() {
        // WHATWG (and so reqwest) treats `\` as `/` in an http URL:
        // the host of each of these is `evil.com`, and the part after
        // the backslash is the path. The old hand-rolled split read
        // the host as `localhost` / `127.0.0.1` and let them through.
        assert!(ensure_https("http://evil.com\\@localhost/dav").is_err());
        assert!(ensure_https("http://evil.com\\@127.0.0.1/").is_err());
        assert!(ensure_https("http://evil.com\\@[::1]/").is_err());
    }

    #[test]
    fn loopback_lookalikes_rejected() {
        assert!(ensure_https("http://localhost.evil.com/").is_err());
        assert!(ensure_https("http://127.0.0.1.evil.com/").is_err());
        assert!(ensure_https("http://127.0.0.2/").is_err());
    }

    #[test]
    fn scheme_and_host_case_do_not_matter() {
        // The parser normalises both, as reqwest will when it connects.
        assert!(ensure_https("HTTPS://example.com/").is_ok());
        assert!(ensure_https("HTTP://LOCALHOST:8080/").is_ok());
        assert!(ensure_https("HTTP://EXAMPLE.COM/").is_err());
    }

    #[test]
    fn other_schemes_rejected() {
        assert!(ensure_https("ftp://example.com").is_err());
        assert!(ensure_https("ws://example.com").is_err());
        assert!(ensure_https("file:///etc/passwd").is_err());
    }

    #[test]
    fn empty_rejected() {
        assert!(ensure_https("").is_err());
    }
}
