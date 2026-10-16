// This is a reader for one sealed archive, not a general tar extractor. Read-only
// inspection found USTAR/00, uid/gid 0, mtime 1790756, no names/device/prefix
// metadata, and exactly two terminal zero blocks. Its 34 local PAX records each
// contain only the next inventory member's long path (including 100-byte names).
// Match those exact bytes; never interpret arbitrary PAX keys or overrides.
use super::{Admission, Evidence, Result, ARCHIVE, RELEASE, hash_file, mkdir, mono_ms, protected};
use flate2::bufread::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
};

const INVENTORY_SHA256: &str = "4fbbe3b1f6a8d8cc98c559c86f629487ed91f5e3daf3cee6f84499d93d7fae26";
const INVENTORY_BYTES: usize = 193_661;
const ARCHIVE_BYTES: u64 = 368_488_700;
const MEMBER_BYTES: u64 = 1_199_734_503;
const MANIFEST_BYTES: usize = 262_144;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Member {
    name: String,
    size: u64,
    mode: String,
    #[serde(rename = "type")]
    kind: String,
    linkname: String,
}

#[derive(Serialize)]
struct Entry<'a> {
    path: &'a str,
    kind: &'static str,
    bytes: u64,
    sha256: Option<String>,
    target: Option<&'a str>,
}

#[derive(Serialize)]
struct Manifest<'a> {
    entries: Vec<Entry<'a>>,
}

fn deadline(end: u64) -> Result<()> {
    if mono_ms()? >= end {
        return Err("sealed extraction preparation deadline".into());
    }
    Ok(())
}

fn exact(r: &mut impl Read, mut b: &mut [u8], end: u64) -> Result<()> {
    while !b.is_empty() {
        deadline(end)?;
        let n = match r.read(b) {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            other => other.map_err(|e| e.to_string())?,
        };
        deadline(end)?;
        if n == 0 {
            return Err("truncated sealed archive".into());
        }
        b = &mut b[n..];
    }
    Ok(())
}

fn clean_path(s: &str) -> bool {
    s.is_ascii()
        && s.len() <= 135
        && !s.bytes().any(|b| b.is_ascii_control() || b == b'\\')
        && s.split('/').all(|c| !matches!(c, "" | "." | ".."))
        && s.split('/').next() == Some(RELEASE)
}

fn copies(name: &str) -> usize {
    if name == RELEASE {
        4
    } else if name.strip_prefix(RELEASE).is_some_and(|p| matches!(p, "/lib" | "/license" | "/plugins")) {
        2
    } else {
        1
    }
}

fn mode(m: &Member) -> Result<u32> {
    match (m.kind.as_str(), m.mode.as_str()) {
        ("5", "0o40755") => Ok(0o40755),
        ("0" | "2", "0o100644") => Ok(0o100644),
        ("0", "0o100755") => Ok(0o100755),
        _ => Err("unsealed archive type/mode".into()),
    }
}

fn link_destination(m: &Member) -> Result<PathBuf> {
    let mut parts: Vec<&str> = m.name.split('/').collect();
    parts.pop();
    if m.linkname.is_empty()
        || !m.linkname.is_ascii()
        || m.linkname.len() > 36
        || m.linkname.bytes().any(|b| b.is_ascii_control() || b == b'\\')
    {
        return Err("invalid sealed link target".into());
    }
    for part in m.linkname.split('/') {
        match part {
            "" | "." => return Err("noncanonical sealed link target".into()),
            ".." if parts.len() > 1 => { parts.pop(); }
            ".." => return Err("sealed link escapes release".into()),
            _ => parts.push(part),
        }
    }
    let path: PathBuf = parts.iter().collect();
    if parts.first().copied() != Some(RELEASE) {
        return Err("sealed link escapes release".into());
    }
    Ok(path)
}

fn validate_members(members: &[Member]) -> Result<usize> {
    let mut unique: BTreeMap<&str, (&Member, usize)> = BTreeMap::new();
    for m in members {
        if !clean_path(&m.name) || m.size > MEMBER_BYTES {
            return Err("invalid sealed member path/size".into());
        }
        mode(m)?;
        if (m.kind != "0" && m.size != 0)
            || (m.kind != "2" && !m.linkname.is_empty())
        {
            return Err("unsealed member payload/link metadata".into());
        }
        if let Some((previous, n)) = unique.get_mut(m.name.as_str()) {
            *n += 1;
            if m.kind != "5" || m != *previous || *n > copies(&m.name) {
                return Err("unsealed duplicate member".into());
            }
        } else {
            unique.insert(&m.name, (m, 1));
        }
    }
    for m in members {
        if m.name != RELEASE {
            let parent = m.name.rsplit_once('/').ok_or("member has no release parent")?.0;
            if !unique.get(parent).is_some_and(|(p, _)| p.kind == "5") {
                return Err("missing directory or link/file member parent".into());
            }
        } else if m.kind != "5" {
            return Err("release root is not a directory".into());
        }
        if m.kind == "2" {
            let target = link_destination(m)?;
            // All frozen links end directly at sealed regular files. Refuse
            // chains instead of introducing general-purpose symlink traversal.
            if !target.to_str().and_then(|p| unique.get(p)).is_some_and(|(p, _)| p.kind == "0") {
                return Err("sealed link target missing or not a regular file".into());
            }
        }
    }
    Ok(unique.len())
}

fn inventory(bytes: &[u8]) -> Result<Vec<Member>> {
    if bytes.len() != INVENTORY_BYTES
        || format!("{:x}", Sha256::digest(bytes)) != INVENTORY_SHA256
    {
        return Err("frozen archive inventory length/hash mismatch".into());
    }
    // Typed deserialization rejects duplicate fields, unknown fields and trailing JSON.
    let members: Vec<Member> = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let unique = validate_members(&members)?;
    let files = members.iter().filter(|m| m.kind == "0").count();
    let links = members.iter().filter(|m| m.kind == "2").count();
    let dirs = members.iter().filter(|m| m.kind == "5").count();
    let bytes = members.iter().try_fold(0_u64, |n, m| n.checked_add(m.size)).ok_or("member size overflow")?;
    let long = members.iter().filter(|m| m.name.len() >= 100).count();
    if members.len() != 1120 || files != 778 || links != 221 || dirs != 121
        || unique != 1114 || bytes != MEMBER_BYTES || long != 34
    {
        return Err("frozen archive inventory counts mismatch".into());
    }
    Ok(members)
}

fn octal(bytes: &mut [u8], mut value: u64) {
    for byte in bytes.iter_mut().rev() {
        *byte = b'0' + (value & 7) as u8;
        value >>= 3;
    }
}

fn checksum(h: &mut [u8; 512]) {
    h[148..156].fill(b' ');
    let sum: u32 = h.iter().map(|b| u32::from(*b)).sum();
    octal(&mut h[148..154], u64::from(sum));
    h[154] = 0;
    h[155] = b' ';
}

fn header(name: &str, mode: u32, size: u64, kind: u8, target: &str) -> Result<[u8; 512]> {
    if !name.is_ascii() || name.len() > 100 || target.len() > 100
        || mode > 0o7777777 || size > 0o77777777777
    {
        return Err("unrepresentable sealed tar header".into());
    }
    let mut h = [0; 512];
    h[..name.len()].copy_from_slice(name.as_bytes());
    octal(&mut h[100..107], u64::from(mode));
    h[107] = b' ';
    h[108..124].copy_from_slice(b"0000000 0000000 ");
    octal(&mut h[124..135], size);
    h[135] = b' ';
    h[136..148].copy_from_slice(b"00006651444 ");
    h[156] = kind;
    h[157..157 + target.len()].copy_from_slice(target.as_bytes());
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    h[329..345].copy_from_slice(b"0000000 0000000 ");
    checksum(&mut h);
    Ok(h)
}

fn expect(r: &mut impl Read, bytes: &[u8], end: u64) -> Result<()> {
    let mut actual = [0; 512];
    if bytes.len() > actual.len() {
        return Err("sealed metadata block limit".into());
    }
    exact(r, &mut actual[..bytes.len()], end)?;
    if &actual[..bytes.len()] != bytes {
        return Err("archive header, metadata, inventory order or padding mismatch".into());
    }
    Ok(())
}

fn padding(r: &mut impl Read, bytes: u64, end: u64) -> Result<()> {
    expect(r, &[0; 512][..((512 - bytes % 512) % 512) as usize], end)
}

fn pax_payload(name: &str) -> String {
    // One fixed path record, not a PAX parser. Decimal length includes itself.
    let body = format!(" path={name}\n");
    let mut len = body.len() + 1;
    loop {
        let next = body.len() + len.to_string().len();
        if next == len {
            return format!("{len}{body}");
        }
        len = next;
    }
}

fn member_header(r: &mut impl Read, m: &Member, end: u64) -> Result<()> {
    if m.name.len() >= 100 {
        let payload = pax_payload(&m.name);
        let mut name = format!("./PaxHeaders.X/{}", m.name.replace('/', "_"));
        name.truncate(99);
        expect(r, &header(&name, 0o100644, payload.len() as u64, b'x', "")?, end)?;
        expect(r, payload.as_bytes(), end)?;
        padding(r, payload.len() as u64, end)?;
    }
    let mut directory_name = [0; 136];
    let name = if m.kind == "5" {
        directory_name[..m.name.len()].copy_from_slice(m.name.as_bytes());
        directory_name[m.name.len()] = b'/';
        std::str::from_utf8(&directory_name[..m.name.len() + 1]).map_err(|_| "non-ASCII sealed directory")?
    } else {
        &m.name
    };
    expect(r, &header(&name[..name.len().min(100)], mode(m)?, m.size, m.kind.as_bytes()[0], &m.linkname)?, end)
}

fn parent_directory(base: &Path, path: &Path) -> Result<()> {
    let owner = fs::symlink_metadata(base).map_err(|e| e.to_string())?.uid();
    let mut current = path.parent().ok_or("extraction parent missing")?;
    loop {
        if !current.starts_with(base) {
            return Err("extraction parent escapes fresh distribution".into());
        }
        let m = fs::symlink_metadata(current).map_err(|e| e.to_string())?;
        if !m.is_dir() || m.file_type().is_symlink() || m.uid() != owner || m.mode() & 0o022 != 0 {
            return Err("extraction parent is not a protected directory".into());
        }
        if current == base {
            return Ok(());
        }
        current = current.parent().ok_or("extraction ancestor missing")?;
    }
}

fn unpack<'a>(r: &mut impl Read, base: &Path, members: &'a [Member], end: u64) -> Result<Manifest<'a>> {
    let mut entries = Vec::with_capacity(members.len());
    let mut seen = BTreeSet::new();
    let mut buffer = [0; 65_536];
    for m in members {
        member_header(r, m, end)?;
        if !seen.insert(m.name.as_str()) {
            // Inventory validation already restricted these to identical frozen directories.
            continue;
        }
        let path = base.join(&m.name);
        parent_directory(base, &path)?;
        let digest = match m.kind.as_str() {
            "5" => { mkdir(&path, 0o700)?; None }
            "2" => None, // No symlink exists while a later payload can still be written.
            "0" => {
                let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600)
                    .open(&path).map_err(|e| e.to_string())?;
                let mut hash = Sha256::new();
                let mut remaining = m.size;
                while remaining != 0 {
                    let n = remaining.min(buffer.len() as u64) as usize;
                    exact(r, &mut buffer[..n], end)?;
                    file.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
                    deadline(end)?;
                    hash.update(&buffer[..n]);
                    remaining -= n as u64;
                }
                padding(r, m.size, end)?;
                file.set_permissions(fs::Permissions::from_mode(mode(m)? & 0o777)).map_err(|e| e.to_string())?;
                file.sync_all().map_err(|e| e.to_string())?;
                deadline(end)?;
                Some(format!("{:x}", hash.finalize()))
            }
            _ => return Err("unsealed archive entry type".into()),
        };
        entries.push(Entry {
            path: &m.name,
            kind: match m.kind.as_str() { "0" => "file", "2" => "symlink", _ => "directory" },
            bytes: m.size,
            sha256: digest,
            target: (m.kind == "2").then_some(m.linkname.as_str()),
        });
    }
    expect(r, &[0; 512], end)?;
    expect(r, &[0; 512], end)?;
    deadline(end)?;
    let mut extra = [0; 1];
    if r.read(&mut extra).map_err(|e| e.to_string())? != 0 {
        return Err("extra unsealed tar data".into());
    }
    deadline(end)?;
    Ok(Manifest { entries })
}

fn unpack_gzip<'a>(
    mut compressed: impl BufRead,
    base: &Path,
    members: &'a [Member],
    end: u64,
) -> Result<Manifest<'a>> {
    deadline(end)?;
    if compressed.fill_buf().map_err(|e| e.to_string())?.get(..10)
        != Some(&[0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 255][..])
    {
        return Err("unsealed gzip header metadata".into());
    }
    let mut gzip = GzDecoder::new(compressed);
    let manifest = unpack(&mut gzip, base, members, end)?;
    let mut compressed = gzip.into_inner();
    if !compressed.fill_buf().map_err(|e| e.to_string())?.is_empty() {
        return Err("trailing or concatenated sealed gzip stream".into());
    }
    deadline(end)?;
    Ok(manifest)
}

fn finish_tree(base: &Path, members: &[Member], end: u64) -> Result<()> {
    for m in members.iter().filter(|m| m.kind == "2") {
        deadline(end)?;
        let path = base.join(&m.name);
        parent_directory(base, &path)?;
        let target = base.join(link_destination(m)?);
        parent_directory(base, &target)?;
        let target_meta = fs::symlink_metadata(&target).map_err(|e| e.to_string())?;
        let owner = fs::symlink_metadata(base).map_err(|e| e.to_string())?.uid();
        if !target_meta.is_file() || target_meta.file_type().is_symlink()
            || target_meta.uid() != owner || target_meta.nlink() != 1 || target_meta.mode() & 0o022 != 0
        {
            return Err("sealed link target substituted".into());
        }
        symlink(&m.linkname, &path).map_err(|e| e.to_string())?;
    }
    for m in members.iter().rev().filter(|m| m.kind == "5") {
        deadline(end)?;
        let path = base.join(&m.name);
        parent_directory(base, &path)?;
        let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err("sealed directory substituted".into());
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }
    fs::set_permissions(base, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    deadline(end)
}

struct ManifestWriter {
    file: BufWriter<File>,
    bytes: usize,
}
impl Write for ManifestWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MANIFEST_BYTES.saturating_sub(self.bytes) {
            return Err(io::Error::other("compact extraction manifest limit"));
        }
        let n = self.file.write(bytes)?;
        self.bytes += n;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> { self.file.flush() }
}

fn sealed_file(path: &Path, bytes: u64) -> Result<File> {
    protected(path, false)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    options.custom_flags(0x800 | 0x20000); // Nonblocking, no final symlink.
    #[cfg(not(target_os = "linux"))]
    options.custom_flags(4 | 0x100);
    let file = options.open(path).map_err(|e| e.to_string())?;
    let m = file.metadata().map_err(|e| e.to_string())?;
    if !m.is_file() || m.uid() != 0 || m.mode() & 0o022 != 0 || m.nlink() != 1 || m.len() != bytes {
        return Err("sealed archive/inventory file identity, ownership or length".into());
    }
    Ok(file)
}

pub(super) fn materialize(dir: &Path, a: &Admission, e: &mut Evidence) -> Result<()> {
    let end = a.preparation_deadline_ms;
    deadline(end)?;
    let inputs = dir.join("inputs");
    protected(&inputs, true)?;
    let mut inventory_file = sealed_file(&inputs.join("archive-members.json"), INVENTORY_BYTES as u64)?;
    let mut bytes = vec![0; INVENTORY_BYTES];
    exact(&mut inventory_file, &mut bytes, end)?;
    let members = inventory(&bytes)?;
    drop(bytes);
    let archive_path = inputs.join("archive.tar.gz");
    let archive = sealed_file(&archive_path, ARCHIVE_BYTES)?;
    // No decompression before the parent seal/deadline/evidence check. Hold the
    // same input inode throughout; protected ancestry excludes non-root swaps.
    hash_file(e, a, &archive_path, ARCHIVE)?;
    let before = archive.metadata().map_err(|e| e.to_string())?;
    let after = fs::symlink_metadata(&archive_path).map_err(|e| e.to_string())?;
    if before.dev() != after.dev() || before.ino() != after.ino()
        || before.len() != after.len() || after.file_type().is_symlink() || after.nlink() != 1
    {
        return Err("archive substituted during seal verification".into());
    }
    let base = inputs.join("distribution");
    mkdir(&base, 0o700)?;
    protected(&base, true)?;
    let manifest = unpack_gzip(BufReader::with_capacity(65_536, archive), &base, &members, end)?;
    finish_tree(&base, &members, end)?;
    deadline(end)?;
    let manifest_path = inputs.join("extraction-manifest.json");
    let file = OpenOptions::new().write(true).create_new(true).mode(0o600)
        .open(&manifest_path).map_err(|e| e.to_string())?;
    let mut writer = ManifestWriter { file: BufWriter::new(file), bytes: 0 };
    serde_json::to_writer(&mut writer, &manifest).map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())?;
    writer.file.get_ref().sync_all().map_err(|e| e.to_string())?;
    deadline(end)?;
    protected(&manifest_path, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Cursor, sync::atomic::{AtomicU64, Ordering}};

    fn member(name: &str, kind: &str, payload: &[u8], target: &str) -> Member {
        Member { name: name.into(), size: payload.len() as u64,
            mode: if kind == "5" { "0o40755" } else { "0o100644" }.into(),
            kind: kind.into(), linkname: target.into() }
    }
    fn records() -> Vec<Member> {
        vec![member(RELEASE, "5", b"", ""),
            member(&format!("{RELEASE}/d"), "5", b"", ""),
            member(&format!("{RELEASE}/f"), "0", b"abc", ""),
            member(&format!("{RELEASE}/d/l"), "2", b"", "../f")]
    }
    fn end() -> u64 { mono_ms().unwrap() + 60_000 }
    fn encoded_header(m: &Member) -> Vec<u8> {
        let mut bytes = Vec::new();
        if m.name.len() >= 100 {
            let payload = pax_payload(&m.name);
            let mut name = format!("./PaxHeaders.X/{}", m.name.replace('/', "_"));
            name.truncate(99);
            bytes.extend(header(&name, 0o100644, payload.len() as u64, b'x', "").unwrap());
            bytes.extend(payload.as_bytes());
            bytes.resize(bytes.len().next_multiple_of(512), 0);
        }
        let name = if m.kind == "5" { format!("{}/", m.name) } else { m.name.clone() };
        bytes.extend(header(&name[..name.len().min(100)], mode(m).unwrap(), m.size, m.kind.as_bytes()[0], &m.linkname).unwrap());
        bytes
    }
    fn tar(members: &[Member]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for m in members {
            bytes.extend(encoded_header(m));
            if m.kind == "0" {
                bytes.extend(b"abc");
                bytes.resize(bytes.len().next_multiple_of(512), 0);
            }
        }
        bytes.extend([0; 1024]);
        bytes
    }
    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::GzBuilder::new().operating_system(255)
            .write(Vec::new(), flate2::Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let p = std::env::temp_dir().join(format!("ktlsp-sealed-materializer-{}-{}-{}",
                std::process::id(), mono_ms().unwrap(), NEXT.fetch_add(1, Ordering::Relaxed)));
            mkdir(&p, 0o700).unwrap();
            Self(p)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }

    #[test]
    fn synthetic_extraction_hashes_files_and_delays_internal_links() {
        let members = records();
        validate_members(&members).unwrap();
        let temp = Temp::new();
        let manifest = unpack(&mut Cursor::new(tar(&members)), &temp.0, &members, end()).unwrap();
        let link = temp.0.join(format!("{RELEASE}/d/l"));
        assert!(fs::symlink_metadata(&link).is_err());
        finish_tree(&temp.0, &members, end()).unwrap();
        assert_eq!(fs::read(&link).unwrap(), b"abc");
        assert_eq!(fs::read_link(link).unwrap(), Path::new("../f"));
        let value: serde_json::Value = serde_json::from_slice(&serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert_eq!(value["entries"].as_array().unwrap().len(), 4);
        assert_eq!(value["entries"][2]["sha256"], "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(value["entries"][3]["target"], "../f");
        assert!(value["entries"][0]["sha256"].is_null());
        assert!(unpack(&mut Cursor::new(tar(&members)), &temp.0, &members, end()).is_err());
    }

    #[test]
    fn sealed_inventory_rejects_traversal_modes_duplicates_and_link_parents() {
        for name in ["/outside", "../outside", "kotlin-server-263.6379.0/../outside", "kotlin-server-263.6379.0//f", "kotlin-server-263.6379.0/./f"] {
            let mut m = records(); m[2].name = name.into();
            assert!(validate_members(&m).is_err());
        }
        for target in ["/outside", "../../outside", "../missing", "../d/l"] {
            let mut m = records(); m[3].linkname = target.into();
            assert!(validate_members(&m).is_err());
        }
        let mut m = records(); m[2].mode = "0o104755".into();
        assert!(validate_members(&m).is_err());
        let mut m = records(); m[2].kind = "1".into();
        assert!(validate_members(&m).is_err());
        let mut m = records(); m.push(member(&format!("{RELEASE}/f"), "0", b"abc", ""));
        assert!(validate_members(&m).is_err());
        let mut m = records(); m.push(member(&format!("{RELEASE}/d/l/child"), "0", b"abc", ""));
        assert!(validate_members(&m).is_err());
        let mut m = records();
        for _ in 0..3 { m.push(member(RELEASE, "5", b"", "")); }
        validate_members(&m).unwrap();
        m.push(member(RELEASE, "5", b"", ""));
        assert!(validate_members(&m).is_err());
    }

    #[test]
    fn exact_headers_reject_even_checksummed_metadata_and_special_entries() {
        let m = &records()[2];
        for (offset, byte) in [(156,b'1'),(156,b'3'),(156,b'x'),(156,b'g'),(156,b'L'),(108,b'1'),(136,b'1'),(345,b'a')] {
            let mut h: [u8;512] = encoded_header(m).try_into().unwrap();
            h[offset] = byte; checksum(&mut h);
            assert!(member_header(&mut Cursor::new(h), m, end()).is_err());
        }
        let mut h = encoded_header(m); h[148] ^= 1;
        assert!(member_header(&mut Cursor::new(h), m, end()).is_err());
        let mut short = encoded_header(m); short.pop();
        assert!(member_header(&mut Cursor::new(short), m, end()).is_err());
    }

    #[test]
    fn only_exact_next_member_path_pax_is_accepted() {
        let name = format!("{RELEASE}/{}", "a".repeat(100 - RELEASE.len() - 1));
        let m = member(&name, "0", b"abc", "");
        member_header(&mut Cursor::new(encoded_header(&m)), &m, end()).unwrap();
        for payload in ["12 size=999\n".to_owned(), "22 linkpath=../../bad\n".to_owned(),
            format!("{}12 size=999\n", pax_payload(&m.name)), pax_payload(&format!("{}b", &m.name[..m.name.len()-1]))] {
            let mut name = format!("./PaxHeaders.X/{}", m.name.replace('/', "_")); name.truncate(99);
            let mut bytes = header(&name,0o100644,payload.len() as u64,b'x',"").unwrap().to_vec();
            bytes.extend(payload.as_bytes()); bytes.resize(bytes.len().next_multiple_of(512),0);
            bytes.extend(&encoded_header(&m)[1024..]);
            assert!(member_header(&mut Cursor::new(bytes), &m, end()).is_err());
        }
        let mut bytes = encoded_header(&m); bytes[1024] ^= 1;
        assert!(member_header(&mut Cursor::new(bytes), &m, end()).is_err());
        let mut bytes = encoded_header(&m); bytes.splice(1024..1024, encoded_header(&m)[..1024].iter().copied());
        assert!(member_header(&mut Cursor::new(bytes), &m, end()).is_err());
    }

    #[test]
    fn gzip_crc_truncation_metadata_and_concatenated_members_refuse() {
        let members = records();
        let encoded = gzip(&tar(&members));
        let temp = Temp::new();
        unpack_gzip(Cursor::new(&encoded), &temp.0, &members, end()).unwrap();
        for bytes in [
            { let mut b = encoded.clone(); let n = b.len(); b[n - 8] ^= 1; b },
            encoded[..encoded.len() - 1].to_vec(),
            { let mut b = encoded.clone(); b.extend(&encoded); b },
            { let mut b = encoded.clone(); b.push(0); b },
            { let mut b = encoded.clone(); b[4] = 1; b },
        ] {
            let temp = Temp::new();
            assert!(unpack_gzip(Cursor::new(bytes), &temp.0, &members, end()).is_err());
            assert!(fs::symlink_metadata(temp.0.join(format!("{RELEASE}/d/l"))).is_err());
        }
    }

    #[test]
    fn exclusive_payload_creation_and_no_link_parents() {
        let temp = Temp::new();
        let release = temp.0.join(RELEASE);
        mkdir(&release, 0o700).unwrap();
        let path = release.join("f");
        fs::write(&path, b"keep").unwrap();
        let members = [member(&format!("{RELEASE}/f"), "0", b"abc", "")];
        assert!(unpack(&mut Cursor::new(tar(&members)), &temp.0, &members, end()).is_err());
        assert_eq!(fs::read(path).unwrap(), b"keep");
        symlink("../", release.join("parent")).unwrap();
        assert!(parent_directory(&temp.0, &release.join("parent/outside")).is_err());
        assert_eq!(&header(&format!("{RELEASE}/"), 0o40755, 0, b'5', "").unwrap()[148..156], b"013470\0 ");
    }

    #[test]
    fn truncated_payload_nonzero_padding_and_extra_tar_data_refuse() {
        let members = records();
        let good = tar(&members);
        for bytes in [good[..2*512+512+2].to_vec(), {
            let mut b=good.clone(); b[3*512+3]=1; b
        }, { let mut b=good.clone(); b.push(0); b }] {
            let temp=Temp::new();
            assert!(unpack(&mut Cursor::new(bytes), &temp.0, &members, end()).is_err());
            assert!(fs::symlink_metadata(temp.0.join(format!("{RELEASE}/d/l"))).is_err());
        }
        let temp=Temp::new();
        assert!(unpack(&mut Cursor::new(good), &temp.0, &members, mono_ms().unwrap()).is_err());
    }
}
