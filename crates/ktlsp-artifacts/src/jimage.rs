//! Selected original JImage 1.0 data. See JIMAGE.md for strict admission and bounds.
use crate::VerifiedArtifact;
use miniz_oxide::inflate::{
    TINFLStatus,
    core::{DecompressorOxide, decompress_with_limit, inflate_flags},
};
use std::{cmp::Ordering, ops::Range};

const MAX_INDEX: usize = 8 * 1024 * 1024;
const MAX_ENTRIES: usize = 16_384;
const MAX_STRINGS: usize = 65_536;
const MAX_NAME: usize = 4096;
const MAX_NAMES: usize = 4 * 1024 * 1024;
const MAX_METADATA: usize = 2 * 1024 * 1024;
const MAX_WORK: usize = 16 * 1024 * 1024;
const MAX_PAYLOAD: usize = 8 * 1024 * 1024;
const MAX_INFLATE_WORK: usize = 32 * 1024 * 1024;
const MAX_LIVE: usize = 2 * MAX_PAYLOAD;
const MAX_LAYERS: usize = 8;
const CHUNK: usize = 32 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JimageError {
    Truncated,
    InvalidMagic,
    UnsupportedVersion(u32),
    UnsupportedFlags(u32),
    Malformed(&'static str),
    Limit(&'static str),
    Allocation,
    UnsupportedCompression,
    InvalidCompressedStream,
    TrailingCompressedInput,
    SizeMismatch,
}
impl std::fmt::Display for JimageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JImage: {self:?}")
    }
}
impl std::error::Error for JimageError {}

type Result<T, E = JimageError> = std::result::Result<T, E>;
#[derive(Clone, Copy, Debug)]
enum Endian {
    Little,
    Big,
}
impl Endian {
    fn u32(self, b: &[u8]) -> u32 {
        let a = [b[0], b[1], b[2], b[3]];
        match self {
            Self::Little => u32::from_le_bytes(a),
            Self::Big => u32::from_be_bytes(a),
        }
    }
    fn u64(self, b: &[u8]) -> u64 {
        let a = [b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]];
        match self {
            Self::Little => u64::from_le_bytes(a),
            Self::Big => u64::from_be_bytes(a),
        }
    }
}

#[derive(Debug)]
struct Entry {
    identity: Vec<u8>,
    module_len: usize,
    slot: u32,
    location: u32,
    location_end: usize,
    span: Range<usize>,
    compressed: bool,
    original_size: usize,
}
impl Entry {
    fn module(&self) -> &[u8] {
        if self.module_len == 0 {
            &[]
        } else {
            &self.identity[1..1 + self.module_len]
        }
    }
    fn resource(&self) -> &[u8] {
        &self.identity[if self.module_len == 0 {
            0
        } else {
            self.module_len + 2
        }..]
    }
}

/// A complete validated index borrowing the whole original verified artifact.
/// Caller provenance remains unverified; no SDK association is inferred.
///
/// ```compile_fail
/// use ktlsp_artifacts::index_jimage;
/// index_jimage(b"arbitrary bytes");
/// ```
/// ```compile_fail
/// use ktlsp_artifacts::{VerifiedArtifact, JimageIndex, index_jimage};
/// fn escape(a: VerifiedArtifact) -> JimageIndex<'static> { index_jimage(&a).unwrap() }
/// ```
/// ```compile_fail
/// use ktlsp_artifacts::{VerifiedArtifact, index_jimage};
/// fn mutate(a: &VerifiedArtifact) {
///     let mut i = index_jimage(a).unwrap();
///     i.entries[0].span = 0..1;
/// }
/// ```
#[derive(Debug)]
pub struct JimageIndex<'a> {
    artifact: &'a VerifiedArtifact,
    endian: Endian,
    strings: &'a [u8],
    string_starts: Vec<u32>,
    entries: Vec<Entry>,
}

/// A private, origin-bound selection; cannot be assembled from a claimed span.
///
/// ```compile_fail
/// use ktlsp_artifacts::JimageResource;
/// let forged = JimageResource { index: todo!(), entry: todo!() };
/// ```
/// ```compile_fail
/// use ktlsp_artifacts::{VerifiedArtifact, JimageResource, index_jimage};
/// fn escape(a: &VerifiedArtifact) -> JimageResource<'_, '_> {
///     let i = index_jimage(a).unwrap();
///     i.select_resource(b"java.base", b"java/lang/Object.class").unwrap().unwrap()
/// }
/// ```
/// ```compile_fail
/// use ktlsp_artifacts::{JimageResource, VerifiedArtifact};
/// fn substitute(r: &mut JimageResource<'_, '_>, a: &VerifiedArtifact) { r.index.artifact = a; }
/// ```
/// ```compile_fail
/// use ktlsp_artifacts::JimageResource;
/// fn escape<'a>(r: &'a JimageResource<'_, '_>) -> &'a [u8] {
///     let mut scratch = Vec::new();
///     r.payload(&mut scratch).unwrap()
/// }
/// ```
/// ```compile_fail
/// use ktlsp_artifacts::JimageResource;
/// fn mutate(r: &JimageResource<'_, '_>) { r.artifact().bytes()[0] = 0; }
/// ```
#[derive(Debug)]
pub struct JimageResource<'i, 'a> {
    index: &'i JimageIndex<'a>,
    entry: &'i Entry,
}
impl<'a> JimageIndex<'a> {
    pub fn artifact(&self) -> &'a VerifiedArtifact {
        self.artifact
    }
    /// Includes structural /modules and /packages records, not a module count.
    pub fn resource_count(&self) -> usize {
        self.entries.len()
    }
    /// Exact MUTF-8 identity, without terminators. No normalization or URI parsing.
    /// Internal directory records are structurally checked but are not module roots.
    pub fn select_resource<'i>(
        &'i self,
        module_mutf8: &[u8],
        resource_mutf8: &[u8],
    ) -> Result<Option<JimageResource<'i, 'a>>> {
        check_name(module_mutf8)?;
        check_name(resource_mutf8)?;
        if module_mutf8.is_empty() || module_mutf8.contains(&b'/') || resource_mutf8.is_empty() {
            return Err(JimageError::Malformed("selection identity"));
        }
        if module_mutf8.len() + resource_mutf8.len() + 2 > MAX_NAME {
            return Err(JimageError::Limit("identity bytes"));
        }
        if matches!(module_mutf8, b"modules" | b"packages") {
            return Ok(None);
        }
        // Entries are sorted by the full encoded identity, not a decoded/normalized name.
        let key = [
            b"/".as_slice(),
            module_mutf8,
            b"/".as_slice(),
            resource_mutf8,
        ];
        let found = self.entries.binary_search_by(|e| {
            e.identity
                .iter()
                .copied()
                .cmp(key.iter().flat_map(|s| s.iter().copied()))
        });
        Ok(found
            .ok()
            .filter(|&n| {
                let entry = &self.entries[n];
                entry.module() == module_mutf8 && entry.resource() == resource_mutf8
            })
            .map(|n| JimageResource {
                index: self,
                entry: &self.entries[n],
            }))
    }
}
impl JimageResource<'_, '_> {
    pub fn artifact(&self) -> &VerifiedArtifact {
        self.index.artifact
    }
    pub fn module_mutf8(&self) -> &[u8] {
        self.entry.module()
    }
    pub fn resource_mutf8(&self) -> &[u8] {
        self.entry.resource()
    }
    pub fn image_identity_mutf8(&self) -> &[u8] {
        &self.entry.identity
    }
    pub fn table_slot(&self) -> u32 {
        self.entry.slot
    }
    pub fn location_offset(&self) -> u32 {
        self.entry.location
    }
    /// Full-file span in the original whole image (compressed envelope if present).
    pub fn stored_span(&self) -> Range<usize> {
        self.entry.span.clone()
    }
    pub fn is_compressed(&self) -> bool {
        self.entry.compressed
    }
    pub fn original_size(&self) -> usize {
        self.entry.original_size
    }
    /// Original bytes borrow the image; transformed output borrows scratch.
    /// All errors empty scratch, including late inner-codec failures.
    pub fn payload<'s>(&'s self, scratch: &'s mut Vec<u8>) -> Result<&'s [u8]> {
        scratch.clear();
        let result = self.decode(scratch);
        if let Err(error) = result {
            scratch.clear();
            return Err(error);
        }
        if self.entry.compressed {
            Ok(scratch)
        } else {
            Ok(&self.index.artifact.bytes()[self.entry.span.clone()])
        }
    }
    fn decode(&self, scratch: &mut Vec<u8>) -> Result<()> {
        cap(scratch.capacity(), MAX_LIVE, "live decode capacity")?;
        cap(self.entry.span.len(), MAX_PAYLOAD, "selected stored bytes")?;
        cap(
            self.entry.original_size,
            MAX_PAYLOAD,
            "selected original bytes",
        )?;
        if !self.entry.compressed {
            return Ok(());
        }
        let mut other = Vec::new();
        let mut work = 0;
        let mut layers = 0;
        // Two output buffers alternate; the original compressed span is borrowed.
        let mut input = &self.index.artifact.bytes()[self.entry.span.clone()];
        loop {
            layers += 1;
            cap(layers, MAX_LAYERS, "compression layers")?;
            let (encoded, expected) = self.index.layer(input)?;
            for amount in [29, encoded.len(), expected, expected] {
                charge(&mut work, amount, MAX_INFLATE_WORK, "decompression work")?;
            }
            reserve_output(scratch, expected, other.capacity())?;
            inflate(encoded, expected, scratch)?;
            if !has_header(self.index.endian, scratch) {
                if scratch.len() != self.entry.original_size {
                    return Err(JimageError::SizeMismatch);
                }
                return Ok(());
            }
            // The terminal byte is validated, never used as loop control.
            std::mem::swap(scratch, &mut other);
            input = &other;
        }
    }
}

fn cap(value: usize, maximum: usize, field: &'static str) -> Result<()> {
    if value > maximum {
        Err(JimageError::Limit(field))
    } else {
        Ok(())
    }
}
fn charge(total: &mut usize, amount: usize, maximum: usize, field: &'static str) -> Result<()> {
    *total = total.checked_add(amount).ok_or(JimageError::Limit(field))?;
    cap(*total, maximum, field)
}
fn reserve<T>(v: &mut Vec<T>, count: usize, metadata: &mut usize) -> Result<()> {
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(JimageError::Limit("metadata capacity"))?;
    cap(
        metadata
            .checked_add(bytes)
            .ok_or(JimageError::Limit("metadata capacity"))?,
        MAX_METADATA,
        "metadata capacity",
    )?;
    v.try_reserve_exact(count)
        .map_err(|_| JimageError::Allocation)?;
    charge(
        metadata,
        v.capacity() * std::mem::size_of::<T>(),
        MAX_METADATA,
        "metadata capacity",
    )
}
fn number(value: u64) -> Result<usize> {
    usize::try_from(value).map_err(|_| JimageError::Malformed("numeric overflow"))
}
fn hash(bytes: &[u8], mut seed: u32) -> u32 {
    for &b in bytes {
        seed = seed.wrapping_mul(0x01000193) ^ u32::from(b);
    }
    seed & 0x7fffffff
}
fn route(identity: &[u8], redirects: &[u8], endian: Endian) -> Result<Option<usize>> {
    let n = redirects.len() / 4;
    if n == 0 {
        return Ok(None);
    }
    let bucket = hash(identity, 0x01000193) as usize % n;
    let r = endian.u32(&redirects[4 * bucket..]) as i32;
    if r == 0 {
        return Ok(None);
    }
    let slot = if r < 0 {
        (-1i64 - i64::from(r)) as usize
    } else {
        hash(identity, r as u32) as usize % n
    };
    if slot >= n {
        return Err(JimageError::Malformed("redirect slot"));
    }
    Ok(Some(slot))
}

/// Validate canonical MUTF-8 (including encoded NUL and unpaired UTF-16 surrogates).
fn check_name(bytes: &[u8]) -> Result<()> {
    cap(bytes.len(), MAX_NAME, "string bytes")?;
    let mut at = 0;
    while at < bytes.len() {
        let a = bytes[at];
        at += 1;
        match a {
            1..=127 => {}
            0xc0..=0xdf => {
                let b = *bytes.get(at).ok_or(JimageError::Malformed("MUTF-8"))?;
                at += 1;
                let u = (u16::from(a & 31) << 6) | u16::from(b & 63);
                if b & 0xc0 != 0x80 || (u < 128 && !(a == 0xc0 && b == 0x80)) {
                    return Err(JimageError::Malformed("MUTF-8"));
                }
            }
            0xe0..=0xef => {
                let b = *bytes.get(at).ok_or(JimageError::Malformed("MUTF-8"))?;
                let c = *bytes.get(at + 1).ok_or(JimageError::Malformed("MUTF-8"))?;
                at += 2;
                let u = (u16::from(a & 15) << 12) | (u16::from(b & 63) << 6) | u16::from(c & 63);
                if b & 0xc0 != 0x80 || c & 0xc0 != 0x80 || u < 2048 {
                    return Err(JimageError::Malformed("MUTF-8"));
                }
            }
            _ => return Err(JimageError::Malformed("MUTF-8")),
        }
    }
    Ok(())
}
fn string_at<'a>(strings: &'a [u8], starts: &[u32], offset: u64) -> Result<&'a [u8]> {
    let off = u32::try_from(offset).map_err(|_| JimageError::Malformed("string offset"))?;
    let n = starts
        .binary_search(&off)
        .map_err(|_| JimageError::Malformed("string boundary"))?;
    let end = starts.get(n + 1).map_or(strings.len(), |&v| v as usize);
    Ok(&strings[off as usize..end - 1])
}

pub fn index_jimage(artifact: &VerifiedArtifact) -> Result<JimageIndex<'_>> {
    let b = artifact.bytes();
    if b.len() < 28 {
        return Err(JimageError::Truncated);
    }
    let endian = match &b[..4] {
        [0xda, 0xda, 0xfe, 0xca] => Endian::Little,
        [0xca, 0xfe, 0xda, 0xda] => Endian::Big,
        _ => return Err(JimageError::InvalidMagic),
    };
    let h = |n| endian.u32(&b[n..]) as usize;
    if h(4) != 0x10000 {
        return Err(JimageError::UnsupportedVersion(h(4) as u32));
    }
    if h(8) != 0 {
        return Err(JimageError::UnsupportedFlags(h(8) as u32));
    }
    let (count, slots, locations_len, strings_len) = (h(12), h(16), h(20), h(24));
    cap(count, MAX_ENTRIES, "resources")?;
    cap(slots, MAX_ENTRIES, "table slots")?;
    if count > slots {
        return Err(JimageError::Malformed("resource count"));
    }
    let locations_start = 28usize
        .checked_add(
            slots
                .checked_mul(8)
                .ok_or(JimageError::Malformed("index overflow"))?,
        )
        .ok_or(JimageError::Malformed("index overflow"))?;
    let strings_start = locations_start
        .checked_add(locations_len)
        .ok_or(JimageError::Malformed("index overflow"))?;
    let data_start = strings_start
        .checked_add(strings_len)
        .ok_or(JimageError::Malformed("index overflow"))?;
    cap(data_start, MAX_INDEX, "index bytes")?;
    if data_start > b.len() {
        return Err(JimageError::Truncated);
    }
    let locations = &b[locations_start..strings_start];
    let strings = &b[strings_start..data_start];
    if locations.first() != Some(&0) || strings.first() != Some(&0) {
        return Err(JimageError::Malformed("zero sentinels"));
    }
    let mut metadata = 0;
    let mut work = 0;
    charge(&mut work, data_start, MAX_WORK, "index work")?;
    // One complete linear string scan; references only use binary-searched boundaries.
    let string_count = strings.iter().filter(|&&v| v == 0).count();
    cap(string_count, MAX_STRINGS, "strings")?;
    let mut string_starts = Vec::new();
    reserve(&mut string_starts, string_count, &mut metadata)?;
    let mut start = 0;
    for (at, &v) in strings.iter().enumerate() {
        if v == 0 {
            check_name(&strings[start..at])?;
            string_starts.push(start as u32);
            start = at + 1;
        }
    }
    if start != strings.len() {
        return Err(JimageError::Malformed("unterminated string"));
    }
    charge(&mut work, strings.len() * 2, MAX_WORK, "index work")?;
    let redirects = &b[28..28 + 4 * slots];
    let offsets = &b[28 + 4 * slots..locations_start];
    for slot in 0..slots {
        let r = endian.u32(&redirects[4 * slot..]) as i32;
        if r < 0 {
            let target = (-1i64 - i64::from(r)) as usize;
            if target >= slots || endian.u32(&offsets[4 * target..]) == 0 {
                return Err(JimageError::Malformed("redirect slot"));
            }
        }
    }
    let mut entries = Vec::new();
    reserve(&mut entries, count, &mut metadata)?;
    let mut name_bytes = 0;
    for slot in 0..slots {
        let location = endian.u32(&offsets[4 * slot..]);
        if location == 0 {
            continue;
        }
        if entries.len() == count {
            return Err(JimageError::Malformed("resource count"));
        }
        let mut at = location as usize;
        let mut values = [0u64; 8];
        let mut seen = 0u8;
        loop {
            charge(&mut work, 1, MAX_WORK, "index work")?;
            let header = *locations
                .get(at)
                .ok_or(JimageError::Malformed("location stream"))?;
            at += 1;
            if header == 0 {
                break;
            }
            let kind = usize::from(header >> 3);
            if !(1..=7).contains(&kind) || seen & (1 << kind) != 0 {
                return Err(JimageError::Malformed("location attribute"));
            }
            seen |= 1 << kind;
            let width = usize::from(header & 7) + 1;
            let data = locations
                .get(at..at + width)
                .ok_or(JimageError::Malformed("location value"))?;
            at += width;
            charge(&mut work, width, MAX_WORK, "index work")?;
            values[kind] = data.iter().fold(0, |v, &byte| (v << 8) | u64::from(byte));
        }
        let mut parts = [&[][..]; 4];
        for n in 0..4 {
            parts[n] = string_at(strings, &string_starts, values[n + 1])?;
        }
        let [module, parent, base, extension] = parts;
        if module.contains(&b'/') || base.is_empty() {
            return Err(JimageError::Malformed("location identity"));
        }
        let length = module.len()
            + parent.len()
            + base.len()
            + extension.len()
            + if module.is_empty() { 0 } else { 2 }
            + usize::from(!parent.is_empty())
            + usize::from(!extension.is_empty());
        cap(length, MAX_NAME, "identity bytes")?;
        charge(
            &mut name_bytes,
            length,
            MAX_NAMES,
            "aggregate identity bytes",
        )?;
        charge(&mut work, length * 4 + 4 * 17, MAX_WORK, "index work")?;
        let mut identity = Vec::new();
        reserve(&mut identity, length, &mut metadata)?;
        if !module.is_empty() {
            identity.push(b'/');
            identity.extend_from_slice(module);
            identity.push(b'/');
        }
        if !parent.is_empty() {
            identity.extend_from_slice(parent);
            identity.push(b'/');
        }
        identity.extend_from_slice(base);
        if !extension.is_empty() {
            identity.push(b'.');
            identity.extend_from_slice(extension);
        }
        if route(&identity, redirects, endian)? != Some(slot) {
            return Err(JimageError::Malformed("hash consistency"));
        }
        let original_size = number(values[7])?;
        let stored_size = number(if values[6] == 0 { values[7] } else { values[6] })?;
        let span_start = data_start
            .checked_add(number(values[5])?)
            .ok_or(JimageError::Malformed("resource span"))?;
        let span_end = span_start
            .checked_add(stored_size)
            .ok_or(JimageError::Malformed("resource span"))?;
        if span_end > b.len() {
            return Err(JimageError::Malformed("resource span"));
        }
        entries.push(Entry {
            identity,
            module_len: module.len(),
            slot: slot as u32,
            location,
            location_end: at,
            span: span_start..span_end,
            compressed: values[6] != 0,
            original_size,
        });
    }
    if entries.len() != count {
        return Err(JimageError::Malformed("resource count"));
    }
    // Numeric sorts have bounded comparisons and no auxiliary allocation.
    charge(&mut work, count * 64, MAX_WORK, "index work")?;
    entries.sort_unstable_by_key(|e| e.location);
    for pair in entries.windows(2) {
        if pair[0].location_end > pair[1].location as usize {
            return Err(JimageError::Malformed("overlapping locations"));
        }
    }
    let mut location_end = 1;
    for e in &entries {
        if locations[location_end..e.location as usize]
            .iter()
            .any(|&v| v != 0)
        {
            return Err(JimageError::Malformed("unindexed location bytes"));
        }
        location_end = e.location_end;
    }
    if locations[location_end..].iter().any(|&v| v != 0) {
        return Err(JimageError::Malformed("unindexed location bytes"));
    }
    entries.sort_unstable_by_key(|e| e.span.start);
    let mut end = data_start;
    for e in &entries {
        if !e.span.is_empty() {
            if e.span.start < end {
                return Err(JimageError::Malformed("overlapping resources"));
            }
            end = e.span.end;
        }
    }
    // Fallible in-place heapsort charges actual compared bytes, including long shared prefixes.
    sort_identities(&mut entries, &mut work)?;
    for pair in entries.windows(2) {
        charge(
            &mut work,
            pair[0].identity.len().min(pair[1].identity.len()),
            MAX_WORK,
            "index work",
        )?;
        if pair[0].identity == pair[1].identity {
            return Err(JimageError::Malformed("duplicate identity"));
        }
    }
    Ok(JimageIndex {
        artifact,
        endian,
        strings,
        string_starts,
        entries,
    })
}
fn compare(a: &Entry, b: &Entry, work: &mut usize) -> Result<Ordering> {
    for (&x, &y) in a.identity.iter().zip(&b.identity) {
        charge(work, 1, MAX_WORK, "index work")?;
        if x != y {
            return Ok(x.cmp(&y));
        }
    }
    Ok(a.identity.len().cmp(&b.identity.len()))
}
fn sort_identities(entries: &mut [Entry], work: &mut usize) -> Result<()> {
    fn sift(entries: &mut [Entry], mut root: usize, end: usize, work: &mut usize) -> Result<()> {
        while root * 2 + 1 < end {
            let mut child = root * 2 + 1;
            if child + 1 < end
                && compare(&entries[child], &entries[child + 1], work)? == Ordering::Less
            {
                child += 1;
            }
            if compare(&entries[root], &entries[child], work)? != Ordering::Less {
                break;
            }
            entries.swap(root, child);
            root = child;
        }
        Ok(())
    }
    for root in (0..entries.len() / 2).rev() {
        sift(entries, root, entries.len(), work)?;
    }
    for end in (1..entries.len()).rev() {
        entries.swap(0, end);
        sift(entries, 0, end, work)?;
    }
    Ok(())
}
fn has_header(endian: Endian, bytes: &[u8]) -> bool {
    bytes.len() >= 29 && endian.u32(bytes) == 0xcafefafa
}
impl JimageIndex<'_> {
    fn layer<'b>(&self, input: &'b [u8]) -> Result<(&'b [u8], usize)> {
        if input.len() < 29 {
            return Err(JimageError::Malformed("compressed header"));
        }
        if !has_header(self.endian, input) {
            return Err(JimageError::Malformed("compressed magic"));
        }
        let size = number(self.endian.u64(&input[4..]))?;
        let expected = number(self.endian.u64(&input[12..]))?;
        cap(size, MAX_PAYLOAD, "compressed layer bytes")?;
        cap(expected, MAX_PAYLOAD, "decoded layer bytes")?;
        if size != input.len() - 29 {
            return Err(JimageError::SizeMismatch);
        }
        if input[28] > 1 {
            return Err(JimageError::Malformed("terminal byte"));
        }
        let codec = string_at(
            self.strings,
            &self.string_starts,
            u64::from(self.endian.u32(&input[20..])),
        )?;
        if codec != b"zip" || self.endian.u32(&input[24..]) != u32::MAX {
            return Err(JimageError::UnsupportedCompression);
        }
        Ok((&input[29..], expected))
    }
}
fn reserve_output(output: &mut Vec<u8>, expected: usize, other_capacity: usize) -> Result<()> {
    output.clear();
    cap(
        other_capacity
            .checked_add(output.capacity().max(expected))
            .ok_or(JimageError::Limit("live decode capacity"))?,
        MAX_LIVE,
        "live decode capacity",
    )?;
    output
        .try_reserve_exact(expected)
        .map_err(|_| JimageError::Allocation)?;
    cap(
        other_capacity
            .checked_add(output.capacity())
            .ok_or(JimageError::Limit("live decode capacity"))?,
        MAX_LIVE,
        "live decode capacity",
    )?;
    output.resize(expected, 0);
    Ok(())
}
fn inflate(input: &[u8], expected: usize, output: &mut [u8]) -> Result<()> {
    let mut state = DecompressorOxide::new();
    let (mut consumed, mut produced) = (0, 0);
    loop {
        let end = (consumed + CHUNK).min(input.len());
        let flags = inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF
            | inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER
            | if end < input.len() {
                inflate_flags::TINFL_FLAG_HAS_MORE_INPUT
            } else {
                0
            };
        let (status, read, written) = decompress_with_limit(
            &mut state,
            &input[consumed..end],
            output,
            produced,
            CHUNK,
            flags,
        );
        consumed += read;
        produced += written;
        match status {
            TINFLStatus::Done => {
                if consumed != input.len() {
                    return Err(JimageError::TrailingCompressedInput);
                }
                if produced != expected {
                    return Err(JimageError::SizeMismatch);
                }
                return Ok(());
            }
            TINFLStatus::HasMoreOutput | TINFLStatus::NeedsMoreInput
                if read != 0 || written != 0 => {}
            _ => return Err(JimageError::InvalidCompressedStream),
        }
    }
}
