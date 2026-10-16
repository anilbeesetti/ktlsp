use ktlsp_artifacts::{
    Artifact, ChecksumStatus, Coordinate, Provenance, VerifiedArtifact, verify_sha256,
};
use sha2::{Digest, Sha256};

pub fn verified(bytes: Vec<u8>) -> VerifiedArtifact {
    let hash = Sha256::digest(&bytes).into();
    verify_sha256(
        Artifact {
            provenance: Provenance {
                coordinate: Coordinate::new("caller.fixture", "opaque-image", "1", None, "bin")
                    .unwrap(),
                repository_root: "caller-supplied-not-opened".into(),
                artifact_path: "not-an-sdk-or-maven-download".into(),
                byte_length: bytes.len(),
                checksum: ChecksumStatus::Unverified,
            },
            bytes,
        },
        hash,
    )
    .unwrap()
}
#[derive(Clone)]
pub struct Record {
    pub module: Vec<u8>,
    pub parent: Vec<u8>,
    pub base: Vec<u8>,
    pub extension: Vec<u8>,
    pub data: Vec<u8>,
    pub compressed: bool,
    pub original: u64,
}
impl Record {
    pub fn stored(module: &[u8], path: &[u8], data: &[u8]) -> Self {
        let (parent, leaf) = match path.iter().rposition(|&v| v == b'/') {
            Some(n) => (&path[..n], &path[n + 1..]),
            None => (&[][..], path),
        };
        let (base, extension) = match leaf.iter().rposition(|&v| v == b'.') {
            Some(n) => (&leaf[..n], &leaf[n + 1..]),
            None => (leaf, &[][..]),
        };
        Self {
            module: module.to_vec(),
            parent: parent.to_vec(),
            base: base.to_vec(),
            extension: extension.to_vec(),
            data: data.to_vec(),
            compressed: false,
            original: data.len() as u64,
        }
    }
}
pub struct Image {
    pub bytes: Vec<u8>,
    pub locations: Vec<usize>,
    pub slots: Vec<usize>,
    pub strings_start: usize,
    pub data_start: usize,
}
fn hash(b: &[u8], seed: u32) -> u32 {
    b.iter()
        .fold(seed, |s, &v| s.wrapping_mul(0x01000193) ^ u32::from(v))
        & 0x7fffffff
}
pub fn image(records: &[Record]) -> Image {
    let count = records.len();
    let mut t = count * 4 + 3;
    while (2..t).any(|n| t % n == 0) {
        t += 1;
    }
    let mut names = Vec::new();
    let mut strings = b"\0zip\0compact-cp\0unknown\0".to_vec();
    let mut intern = std::collections::BTreeMap::<Vec<u8>, u64>::new();
    let mut attrs = Vec::new();
    let mut payload = Vec::new();
    for r in records {
        let mut values = [0u64; 7];
        for (n, part) in [&r.module, &r.parent, &r.base, &r.extension]
            .into_iter()
            .enumerate()
        {
            if !part.is_empty() {
                values[n] = *intern.entry(part.clone()).or_insert_with(|| {
                    let offset = strings.len() as u64;
                    strings.extend(part);
                    strings.push(0);
                    offset
                });
            }
        }
        let mut name = Vec::new();
        if !r.module.is_empty() {
            name.push(b'/');
            name.extend(&r.module);
            name.push(b'/');
        }
        if !r.parent.is_empty() {
            name.extend(&r.parent);
            name.push(b'/');
        }
        name.extend(&r.base);
        if !r.extension.is_empty() {
            name.push(b'.');
            name.extend(&r.extension);
        }
        names.push(name);
        values[4] = payload.len() as u64;
        values[5] = if r.compressed { r.data.len() as u64 } else { 0 };
        values[6] = r.original;
        payload.extend(&r.data);
        attrs.push(values);
    }
    let mut groups = vec![Vec::new(); t];
    for (n, name) in names.iter().enumerate() {
        groups[hash(name, 0x01000193) as usize % t].push(n);
    }
    let mut order: Vec<usize> = (0..t).collect();
    order.sort_by_key(|&n| std::cmp::Reverse(groups[n].len()));
    let mut redirects = vec![0i32; t];
    let mut occupied = vec![false; t];
    let mut slots = vec![0; count];
    for &bucket in &order {
        if groups[bucket].len() < 2 {
            continue;
        }
        let mut seed = 1;
        loop {
            let candidates: Vec<_> = groups[bucket]
                .iter()
                .map(|&n| hash(&names[n], seed) as usize % t)
                .collect();
            if candidates.iter().all(|&n| !occupied[n])
                && candidates
                    .iter()
                    .enumerate()
                    .all(|(n, v)| !candidates[..n].contains(v))
            {
                for (&record, &slot) in groups[bucket].iter().zip(&candidates) {
                    slots[record] = slot;
                    occupied[slot] = true;
                }
                redirects[bucket] = seed as i32;
                break;
            }
            seed += 1;
            assert!(seed < 1_000_000, "authored perfect hash seed");
        }
    }
    for bucket in order {
        if groups[bucket].len() == 1 {
            let slot = occupied.iter().position(|&v| !v).unwrap();
            slots[groups[bucket][0]] = slot;
            occupied[slot] = true;
            redirects[bucket] = -1 - slot as i32;
        }
    }
    let locations_start = 28 + t * 8;
    let locations_len = 1 + count * 64;
    let strings_start = locations_start + locations_len;
    let data_start = strings_start + strings.len();
    let mut bytes = Vec::new();
    for v in [
        0xcafedada,
        0x10000,
        0,
        count as u32,
        t as u32,
        locations_len as u32,
        strings.len() as u32,
    ] {
        bytes.extend(v.to_le_bytes());
    }
    for r in redirects {
        bytes.extend(r.to_le_bytes());
    }
    let mut offsets = vec![0u32; t];
    for (n, &slot) in slots.iter().enumerate() {
        offsets[slot] = (1 + 64 * n) as u32;
    }
    for off in offsets {
        bytes.extend(off.to_le_bytes());
    }
    bytes.push(0);
    let mut locations = Vec::new();
    for values in attrs {
        locations.push(bytes.len());
        for (n, value) in values.iter().enumerate() {
            bytes.push((((n + 1) << 3) | 7) as u8);
            bytes.extend(value.to_be_bytes());
        }
        bytes.push(0);
    }
    bytes.extend(strings);
    bytes.extend(payload);
    Image {
        bytes,
        locations,
        slots,
        strings_start,
        data_start,
    }
}
/// Authored zlib stored blocks, independent of the production inflater.
pub fn zip_layer(data: &[u8], terminal: u8, plugin: u32) -> Vec<u8> {
    let mut z = vec![0x78, 0x01];
    let chunks = data.len().max(1).div_ceil(65_535);
    for n in 0..chunks {
        let start = (n * 65_535).min(data.len());
        let end = ((n + 1) * 65_535).min(data.len());
        let length = (end - start) as u16;
        z.push(u8::from(n + 1 == chunks));
        z.extend(length.to_le_bytes());
        z.extend((!length).to_le_bytes());
        z.extend(&data[start..end]);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &v in data {
        a = (a + u32::from(v)) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend(((b << 16) | a).to_be_bytes());
    let mut out = Vec::new();
    out.extend(0xcafefafau32.to_le_bytes());
    out.extend((z.len() as u64).to_le_bytes());
    out.extend((data.len() as u64).to_le_bytes());
    out.extend(plugin.to_le_bytes());
    out.extend(u32::MAX.to_le_bytes());
    out.push(terminal);
    out.extend(z);
    out
}
