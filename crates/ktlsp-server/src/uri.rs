//! Generic absolute RFC 3986 URI validation; identity remains the wire spelling.
use std::net::Ipv6Addr;

fn unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"-._~".contains(&b)
}
fn subdelim(b: u8) -> bool {
    b"!$&'()*+,;=".contains(&b)
}
fn component(value: &str, extra: &[u8]) -> bool {
    let bytes = value.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let b = bytes[index];
        if b == b'%' {
            if index + 2 >= bytes.len()
                || !bytes[index + 1..index + 3]
                    .iter()
                    .all(u8::is_ascii_hexdigit)
            {
                return false;
            }
            index += 3;
        } else if unreserved(b) || subdelim(b) || extra.contains(&b) {
            index += 1;
        } else {
            return false;
        }
    }
    true
}
fn authority(value: &str) -> bool {
    let host_port = if let Some((user, host)) = value.split_once('@') {
        if !component(user, b":") {
            return false;
        }
        host
    } else {
        value
    };
    if let Some(rest) = host_port.strip_prefix('[') {
        let Some((host, suffix)) = rest.split_once(']') else {
            return false;
        };
        let ipv_future = host
            .strip_prefix('v')
            .or_else(|| host.strip_prefix('V'))
            .is_some_and(|rest| {
                rest.split_once('.').is_some_and(|(version, address)| {
                    !version.is_empty()
                        && version.bytes().all(|b| b.is_ascii_hexdigit())
                        && !address.is_empty()
                        && address
                            .bytes()
                            .all(|b| unreserved(b) || subdelim(b) || b == b':')
                })
            });
        (host.parse::<Ipv6Addr>().is_ok() || ipv_future)
            && (suffix.is_empty()
                || suffix
                    .strip_prefix(':')
                    .is_some_and(|port| port.bytes().all(|b| b.is_ascii_digit())))
    } else {
        let (host, port) = host_port
            .split_once(':')
            .map_or((host_port, None), |(h, p)| (h, Some(p)));
        component(host, b"") && port.is_none_or(|p| p.bytes().all(|b| b.is_ascii_digit()))
    }
}

pub(crate) fn absolute(value: &str) -> bool {
    let Some((scheme, remainder)) = value.split_once(':') else {
        return false;
    };
    if !scheme
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        || !scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(&b))
    {
        return false;
    }
    let (before_fragment, fragment) = remainder
        .split_once('#')
        .map_or((remainder, None), |(a, b)| (a, Some(b)));
    let (hierarchy, query) = before_fragment
        .split_once('?')
        .map_or((before_fragment, None), |(a, b)| (a, Some(b)));
    if query.is_some_and(|q| !component(q, b":@/?"))
        || fragment.is_some_and(|f| !component(f, b":@/?"))
    {
        return false;
    }
    if let Some(rest) = hierarchy.strip_prefix("//") {
        let (auth, path) = rest.split_once('/').map_or((rest, ""), |(a, p)| (a, p));
        authority(auth) && component(path, b":@/")
    } else {
        component(hierarchy, b":@/")
    }
}
