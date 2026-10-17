//! Pure local-file URI boundary, separate from unrelated exact-wire document URIs.
//!
//! No filesystem access or physical-identity/case-equivalence proof is performed.
//! Only native absolute UTF-8 paths are admitted, within the server URI byte limit
//! (including canonical escaped output). Dot components, network/device paths and
//! ambiguous foreign forms refuse rather than acquiring filesystem authority.
use std::path::{Component, Path, PathBuf};

use crate::MAX_URI_BYTES;

/// Decode a local `file` URI exactly once, without touching the filesystem.
///
/// The scheme is ASCII case-insensitive; only empty authority or literal
/// `localhost` is local. URI path syntax is ASCII (Unicode must be UTF-8 escaped).
/// Encoded separators, queries/fragments, traversal and invalid UTF-8 refuse.
/// Redundant non-leading separators and a trailing separator are harmless aliases;
/// `encode` removes them. No Unicode normalization or filesystem case folding occurs.
pub(crate) fn decode(uri: &str) -> Result<PathBuf, &'static str> {
    if uri.len() > MAX_URI_BYTES {
        return Err("local file URI exceeds byte limit");
    }
    let (scheme, remainder) = uri.split_once(':').ok_or("missing file URI scheme")?;
    if !scheme.eq_ignore_ascii_case("file") {
        return Err("expected local file URI");
    }
    let path = if let Some(hierarchy) = remainder.strip_prefix("//") {
        let (authority, _) = hierarchy.split_once('/').ok_or("missing absolute file path")?;
        if !authority.is_empty() && authority != "localhost" {
            return Err("nonlocal file URI authority");
        }
        &hierarchy[authority.len()..]
    } else {
        remainder
    };
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'%' {
            let high = bytes.get(index + 1).and_then(|b| hex(*b));
            let low = bytes.get(index + 2).and_then(|b| hex(*b));
            let value = match (high, low) {
                (Some(high), Some(low)) => high * 16 + low,
                _ => return Err("malformed file URI escape"),
            };
            if matches!(value, 0 | b'/' | b'\\') {
                return Err("encoded separator or NUL in file URI");
            }
            decoded.push(value);
            index += 3;
        } else {
            if !unreserved(byte) && !b"!$&'()*+,;=:@/".contains(&byte) {
                return Err("invalid file URI path syntax");
            }
            decoded.push(byte);
            index += 1;
        }
    }
    let decoded = String::from_utf8(decoded).map_err(|_| "file URI path is not UTF-8")?;
    decoded_path(decoded)
}

/// Encode a native absolute UTF-8 path into bounded canonical ASCII `file:///…`.
///
/// Escapes use uppercase UTF-8 byte hex; only unreserved bytes and separators
/// remain literal (plus a Windows drive colon). Windows Disk and canonical
/// VerbatimDisk prefixes are supported, with ASCII drive letters uppercased.
/// UNC/device prefixes, ADS, reserved Windows names and trailing dots/spaces
/// refuse on Windows. POSIX refuses Windows drive forms and backslashes.
/// This is lexical validation, not filesystem containment or alias authentication.
pub(crate) fn encode(path: &Path) -> Result<String, &'static str> {
    let raw = path.to_str().ok_or("native path is not UTF-8")?;
    if raw.len() > MAX_URI_BYTES {
        return Err("local file path exceeds byte limit");
    }
    validate_native(path, raw)?;
    let mut uri = String::with_capacity(raw.len().saturating_add(7).min(MAX_URI_BYTES));
    uri.push_str("file://");
    for component in path.components() {
        match component {
            #[cfg(windows)]
            Component::Prefix(prefix) => {
                use std::path::Prefix;
                let drive = match prefix.kind() {
                    Prefix::Disk(drive) | Prefix::VerbatimDisk(drive) => drive,
                    _ => return Err("unsupported native file prefix"),
                };
                uri.push('/');
                uri.push(char::from(drive.to_ascii_uppercase()));
                uri.push(':');
            }
            Component::RootDir => uri.push('/'),
            Component::Normal(value) => {
                let value = value.to_str().ok_or("native path is not UTF-8")?;
                if !uri.ends_with('/') {
                    if uri.len() == MAX_URI_BYTES {
                        return Err("canonical file URI exceeds byte limit");
                    }
                    uri.push('/');
                }
                for byte in value.bytes() {
                    let needed = if unreserved(byte) { 1 } else { 3 };
                    if needed > MAX_URI_BYTES - uri.len() {
                        return Err("canonical file URI exceeds byte limit");
                    }
                    if unreserved(byte) {
                        uri.push(char::from(byte));
                    } else {
                        const HEX: &[u8; 16] = b"0123456789ABCDEF";
                        uri.push('%');
                        uri.push(char::from(HEX[usize::from(byte >> 4)]));
                        uri.push(char::from(HEX[usize::from(byte & 15)]));
                    }
                }
            }
            _ => return Err("unsupported native file component"),
        }
    }
    Ok(uri)
}

fn unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-._~".contains(&byte)
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn absolute_slash_path(path: &str) -> Result<(), &'static str> {
    if !path.starts_with('/') || path.starts_with("//") {
        return Err("expected absolute non-network file path");
    }
    if path.contains(['\0', '\\']) {
        return Err("NUL or backslash in file path");
    }
    if path.split('/').any(|part| matches!(part, "." | "..")) {
        return Err("dot component in file path");
    }
    Ok(())
}

#[cfg(not(windows))]
fn validate_native(_path: &Path, raw: &str) -> Result<(), &'static str> {
    absolute_slash_path(raw)?;
    let first = raw[1..].split('/').next().unwrap_or("").as_bytes();
    if first.len() >= 2 && first[0].is_ascii_alphabetic() && matches!(first[1], b':' | b'|') {
        return Err("foreign Windows drive path");
    }
    Ok(())
}

#[cfg(not(windows))]
fn decoded_path(decoded: String) -> Result<PathBuf, &'static str> {
    validate_native(Path::new(&decoded), &decoded)?;
    Ok(PathBuf::from(decoded))
}

#[cfg(windows)]
fn decoded_path(decoded: String) -> Result<PathBuf, &'static str> {
    absolute_slash_path(&decoded)?;
    let bytes = decoded.as_bytes();
    if bytes.len() < 4 || !bytes[1].is_ascii_alphabetic() || bytes[2..4] != *b":/" {
        return Err("expected absolute Windows drive file URI");
    }
    windows_components(decoded[4..].split('/'))?;
    let mut native = String::with_capacity(decoded.len() - 1);
    native.push(char::from(bytes[1].to_ascii_uppercase()));
    native.push(':');
    for character in decoded[3..].chars() {
        native.push(if character == '/' { '\\' } else { character });
    }
    Ok(PathBuf::from(native))
}

#[cfg(windows)]
fn validate_native(path: &Path, raw: &str) -> Result<(), &'static str> {
    use std::path::Prefix;
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return Err("expected native Windows drive path");
    };
    let remainder = match prefix.kind() {
        Prefix::Disk(drive) if drive.is_ascii_alphabetic() => &raw[2..],
        Prefix::VerbatimDisk(drive)
            if drive.is_ascii_alphabetic() && raw.starts_with(r"\\?\") && !raw.contains('/') =>
        {
            &raw[6..]
        }
        _ => return Err("unsupported native Windows file prefix"),
    };
    if !path.is_absolute() || !remainder.starts_with(['/', '\\']) {
        return Err("expected absolute Windows drive path");
    }
    windows_components(remainder.split(['/', '\\']))
}

#[cfg(windows)]
fn windows_components<'a>(parts: impl Iterator<Item = &'a str>) -> Result<(), &'static str> {
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if part.ends_with(['.', ' '])
            || part.chars().any(|c| c <= '\u{1f}' || "<>:\"/\\|?*".contains(c))
        {
            return Err("ambiguous or invalid Windows file component");
        }
        let base = part.split('.').next().unwrap_or("").trim_end_matches(' ');
        if ["CON", "PRN", "AUX", "NUL", "CLOCK$", "CONIN$", "CONOUT$"]
            .iter()
            .any(|name| base.eq_ignore_ascii_case(name))
            || (base.get(..3).is_some_and(|prefix| {
                prefix.eq_ignore_ascii_case("COM") || prefix.eq_ignore_ascii_case("LPT")
            }) && matches!(base.get(3..), Some("1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")))
        {
            return Err("reserved Windows device component");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    const ROOT: &str = "/";
    #[cfg(windows)]
    const ROOT: &str = r"C:\";
    #[cfg(not(windows))]
    const ROOT_URI: &str = "file:///";
    #[cfg(windows)]
    const ROOT_URI: &str = "file:///C:/";

    fn file(name: &str) -> String {
        format!("{ROOT_URI}{name}")
    }

    #[test]
    fn refuses_invalid_wire_paths_without_panics() {
        for uri in [
            "", "file:", "file:relative", "file://", "file://localhost", "file://remote/a",
            "file://LOCALHOST/a", "file://localhost:80/a", "file://user@localhost/a",
            "file://%6cocalhost/a", "file:////host/share", "file://[::1]/a", "https:///a",
        ] {
            assert!(decode(uri).is_err(), "accepted {uri:?}");
        }
        for suffix in [
            "%", "%0", "%GG", "%0g", "%00", "%2f", "%2F", "%5c", "%5C", "%FF",
            "%C0%AF", "%ED%A0%80", "%F4%90%80%80", "%E2%82", "a?x", "a#x", "a\\b",
            "a b", "é", "\n", "\0", ".", "..", "%2e", "%2E%2e", "a/../b", "a/./b",
        ] {
            let uri = file(suffix);
            assert!(decode(&uri).is_err(), "accepted {uri:?}");
        }
    }

    #[test]
    fn canonical_aliases_and_exactly_once_percent_decoding() {
        let canonical = file("a%20b/%C3%A9%F0%9F%98%80%25.kt");
        let alias = canonical.replacen("file:///", "FiLe://localhost/", 1)
            .replace("%C3%A9", "%c3%a9");
        assert_eq!(encode(&decode(&alias).unwrap()).unwrap(), canonical);
        assert_eq!(encode(&decode(&file("%252e%252e.kt")).unwrap()).unwrap(), file("%252e%252e.kt"));
        assert_eq!(encode(&decode(&file("%252F%255c%2500")).unwrap()).unwrap(), file("%252F%255c%2500"));
        assert_eq!(encode(&decode(&file("%61//b/")).unwrap()).unwrap(), file("a/b"));
        assert_eq!(encode(&decode(ROOT_URI).unwrap()).unwrap(), ROOT_URI);
    }

    #[test]
    fn enforces_wire_and_expanded_output_byte_boundaries() {
        let count = MAX_URI_BYTES - ROOT_URI.len();
        let boundary = file(&"a".repeat(count));
        assert_eq!(boundary.len(), MAX_URI_BYTES);
        assert_eq!(encode(&decode(&boundary).unwrap()).unwrap(), boundary);
        assert!(decode(&(boundary + "a")).is_err());
        assert!(encode(&Path::new(ROOT).join("a".repeat(count + 1))).is_err());
        assert!(encode(&Path::new(ROOT).join("a".repeat(MAX_URI_BYTES))).is_err());
        let escapes = (count - 1) / 3;
        let tail = count - escapes * 3;
        let path = Path::new(ROOT).join(format!("{}{}", " ".repeat(escapes), "a".repeat(tail)));
        let expected = file(&format!("{}{}", "%20".repeat(escapes), "a".repeat(tail)));
        assert_eq!(expected.len(), MAX_URI_BYTES);
        assert_eq!(encode(&path).unwrap(), expected);
        assert_eq!(decode(&expected).unwrap(), path);
        assert!(encode(&Path::new(ROOT).join(format!("{}a", " ".repeat(escapes + 1)))).is_err());
    }

    #[cfg(not(windows))]
    #[test]
    fn posix_literal_paths_and_foreign_forms() {
        assert_eq!(decode("file:/tmp/a%20b/%C3%A9%F0%9F%98%80%25.kt").unwrap(), PathBuf::from("/tmp/a b/é😀%.kt"));
        assert_eq!(encode(Path::new("/tmp/a b/é😀%.kt")).unwrap(), "file:///tmp/a%20b/%C3%A9%F0%9F%98%80%25.kt");
        assert_eq!(encode(Path::new("/tmp/a:b?#\n.kt")).unwrap(), "file:///tmp/a%3Ab%3F%23%0A.kt");
        assert_eq!(decode("file:///tmp/a%3Ab%3F%23%0A.kt").unwrap(), PathBuf::from("/tmp/a:b?#\n.kt"));
        for path in ["relative", "C:/x", "/C:/x", "/c:relative", "/C|/x", "//host/share", "/a/../b", "/a/./b", "/a\\b", "/a\0b"] {
            assert!(encode(Path::new(path)).is_err(), "accepted {path:?}");
        }
        for uri in ["file:///C:/x", "file:///c%3A/x", "file:///C%7C/x"] {
            assert!(decode(uri).is_err(), "accepted {uri:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn refuses_non_utf8_native_paths() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        assert!(encode(Path::new(OsStr::from_bytes(b"/tmp/\xff"))).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn windows_real_disk_and_verbatim_paths() {
        let expected = "file:///C:/a%20b/%C3%A9%F0%9F%98%80%25.kt";
        assert_eq!(decode(expected).unwrap(), PathBuf::from("C:\\a b\\é😀%.kt"));
        assert_eq!(encode(Path::new("c:\\a b\\é😀%.kt")).unwrap(), expected);
        assert_eq!(encode(Path::new("\\\\?\\C:\\a b\\é😀%.kt")).unwrap(), expected);
        assert_eq!(encode(Path::new("C:/a//b/")).unwrap(), "file:///C:/a/b");
        assert_eq!(encode(&decode("file:/c%3a/a").unwrap()).unwrap(), "file:///C:/a");
        assert_eq!(encode(Path::new(r"\\?\C:\")).unwrap(), "file:///C:/");
        for uri in ["file:///tmp/a", "file:///C:relative", "file:///C|/a", "file:///C:/a:b", "file:///C:/a%3Ab", "file:///C:/a%3Fb", "file:///C:/a%00b"] {
            assert!(decode(uri).is_err(), "accepted {uri:?}");
        }
        for path in [r"C:relative", r"\rooted", r"\\host\share\a", r"\\?\UNC\host\share\a", r"\\.\C:\a", r"\\?\GLOBALROOT\a", r"\\?\C:\a/b", r"C:\a\..\b", r"C:\a\.\b"] {
            assert!(encode(Path::new(path)).is_err(), "accepted {path:?}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_refuses_devices_ads_and_ambiguous_components() {
        for component in ["CON", "con.kt", "CON .txt", "NUL", "AUX.txt", "PRN", "COM1", "Lpt9.kt", "COM¹", "LPT².txt", "COM³", "CLOCK$", "CONIN$", "CONOUT$", "a.", "a ", ".", "..", "a:b", "a<", "a>", "a\"", "a|", "a?", "a*", "a\u{1f}"] {
            assert!(encode(&Path::new(ROOT).join(component)).is_err(), "accepted {component:?}");
        }
        for suffix in ["con.kt", "CON%20.txt", "COM%C2%B9", "a%20", "a.", "a%3Ab", "a%3F", "a%01", "x/NUL/y"] {
            assert!(decode(&file(suffix)).is_err(), "accepted {suffix:?}");
        }
        assert_eq!(encode(Path::new(r"C:\COM10\console.kt")).unwrap(), "file:///C:/COM10/console.kt");
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        let invalid = OsString::from_wide(&[67, 58, 92, 0xd800]);
        assert!(encode(Path::new(&invalid)).is_err());
    }
}
