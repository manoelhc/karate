//! Conservative URL/address policy, not a network authorization or DNS resolver.
use crate::Error;
use std::net::IpAddr;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    origin: String,
    path_prefix: String,
    blocked_paths: Vec<String>,
}

impl Scope {
    pub fn new(input: &str, blocked_paths: Vec<String>) -> Result<Self, Error> {
        let url = parse_unambiguous(input).ok_or(Error::Validation)?;
        if url.query().is_some()
            || blocked_paths.len() > 100
            || blocked_paths.iter().any(|path| !valid_path(path))
        {
            return Err(Error::Validation);
        }
        Ok(Self {
            origin: url.origin().ascii_serialization(),
            path_prefix: url.path().trim_end_matches('/').to_owned(),
            blocked_paths,
        })
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub fn permits(&self, input: &str) -> Result<Url, Error> {
        let url = parse_unambiguous(input).ok_or(Error::OutOfScope)?;
        if url.origin().ascii_serialization() != self.origin
            || !within(url.path(), &self.path_prefix)
            || self
                .blocked_paths
                .iter()
                .any(|path| within(url.path(), path.trim_end_matches('/')))
        {
            return Err(Error::OutOfScope);
        }
        Ok(url)
    }
}

fn parse_unambiguous(input: &str) -> Option<Url> {
    // Check raw input first: the WHATWG parser removes dot segments, controls
    // and backslashes before returning Url::path(). Never authorize that repair.
    if input.len() > 8192
        || input.chars().any(|c| c.is_whitespace() || c.is_control())
        || input.contains('\\')
    {
        return None;
    }
    let (scheme, rest) = input.split_once("://")?;
    if !matches!(scheme, "http" | "https") {
        return None;
    }
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    if authority.is_empty() || authority.contains(['@', '%']) {
        return None;
    }
    let tail = &rest[authority_end..];
    let path = tail.split(['?', '#']).next()?;
    if !path.is_empty() && !valid_path(path) {
        return None;
    }
    let url = Url::parse(input).ok()?;
    if url.host_str().is_none() || url.fragment().is_some() || !valid_path(url.path()) {
        return None;
    }
    Some(url)
}

fn valid_path(path: &str) -> bool {
    path.starts_with('/')
        && path.len() <= 2048
        && path.is_ascii()
        && !path.contains(['\\', '%', '?', '#', ';'])
        && !path.contains("//")
        && !path.chars().any(|c| c.is_whitespace() || c.is_control())
        && !path.split('/').any(|segment| matches!(segment, "." | ".."))
}

fn within(path: &str, prefix: &str) -> bool {
    prefix.is_empty()
        || path == prefix
        || path
            .strip_prefix(prefix)
            .is_some_and(|tail| tail.starts_with('/'))
}

/// All DNS answers must pass, then the transport must pin a checked address.
/// This does NOT authorize a public destination or follow redirects.
pub fn public_addresses(addresses: &[IpAddr]) -> Result<(), Error> {
    if addresses.is_empty() || addresses.len() > 64 || addresses.iter().any(|ip| !public_ip(*ip)) {
        Err(Error::OutOfScope)
    } else {
        Ok(())
    }
}

/// Deliberately conservative special-use exclusion; private connectors are separate.
pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && (b == 168 || b == 0 || (b == 88 && c == 99)))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            (s[0] & 0xe000) == 0x2000
                && !(s[0] == 0x2001 && (s[1] < 0x200 || s[1] == 0xdb8))
                && s[0] != 0x2002
                && !(s[0] == 0x3fff && s[1] < 0x1000)
        }
    }
}
