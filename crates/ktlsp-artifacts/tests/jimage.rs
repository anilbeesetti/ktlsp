#[path = "fixtures/jimage/authored.rs"]
mod authored;
use authored::{Record, image, verified, zip_layer};
use ktlsp_artifacts::{ChecksumStatus, JimageError as E, index_jimage};
use sha2::{Digest, Sha256};

const PLAIN_LE: &[u8] = include_bytes!("fixtures/jimage/non-compressed_little-endian.jimage");
const PLAIN_BE: &[u8] = include_bytes!("fixtures/jimage/non-compressed_big-endian.jimage");
const ZIP_LE: &[u8] = include_bytes!("fixtures/jimage/compressed_little-endian.jimage");
const ZIP_BE: &[u8] = include_bytes!("fixtures/jimage/compressed_big-endian.jimage");
const OBJECT: &[u8] = include_bytes!("fixtures/jimage/Object.class.bin");
const MAX: usize = 8 * 1024 * 1024;
fn digest(b: &[u8]) -> String { format!("{:x}", Sha256::digest(b)) }
fn put32(b: &mut [u8], at: usize, value: u32) { b[at..at + 4].copy_from_slice(&value.to_le_bytes()); }
fn attr(b: &mut [u8], at: usize, kind: usize, value: u64) { b[at + (kind - 1) * 9 + 1..at + kind * 9].copy_from_slice(&value.to_be_bytes()); }
fn rejected(bytes: Vec<u8>) -> E { index_jimage(&verified(bytes)).unwrap_err() }
fn compressed(data: Vec<u8>, original: usize) -> Record {
    let mut r = Record::stored(b"m", b"p/resource", &data); r.compressed = true; r.original = original as u64; r
}
fn payload_error(record: Record) -> E {
    let a = verified(image(&[record]).bytes);
    let i = index_jimage(&a).unwrap();
    let r = i.select_resource(b"m", b"p/resource").unwrap().unwrap();
    let mut scratch = vec![0xaa; 31];
    let e = r.payload(&mut scratch).unwrap_err();
    assert!(scratch.is_empty(), "partial payload escaped on {e:?}"); e
}

#[test]
fn genuine_both_orders_original_payloads_and_whole_image_authority() {
    for (b, hash, compressed, span, loc) in [
        (PLAIN_LE, "6ec5a68e05f2e4f2b6eec5b88277c8ff3b97daf2cb4d66795be2e1fd74f332a8", false, 7109..7269, 63),
        (PLAIN_BE, "9d962f97d8ba5f7b36d92e4673fedc67af653c1eff068bec36f27b5ebd4ff7a5", false, 7109..7269, 63),
        (ZIP_LE, "597286cc0850f048f02ef321ea1625eed7fb867d7d7ed9c791b23150e2004bfb", true, 3142..3306, 67),
        (ZIP_BE, "11c975ddb80f830008106601f53053c2ad263ce2996e6a669e5da3a8f6698b5c", true, 3142..3306, 67),
    ] {
        assert_eq!(digest(b), hash);
        let a = verified(b.to_vec());
        let i = index_jimage(&a).unwrap();
        assert_eq!(i.resource_count(), 12);
        assert!(std::ptr::eq(i.artifact(), &a));
        let r = i.select_resource(b"java.base", b"java/lang/Object.class").unwrap().unwrap();
        assert!(std::ptr::eq(r.artifact(), &a));
        assert!(std::ptr::eq(r.artifact().provenance(), a.provenance()));
        assert_eq!(r.artifact().provenance().checksum, ChecksumStatus::Unverified);
        assert_eq!(r.artifact().digest(), a.digest());
        assert_eq!((r.table_slot(), r.location_offset()), (6, loc));
        assert_eq!(r.stored_span(), span.clone());
        assert_eq!(r.image_identity_mutf8(), b"/java.base/java/lang/Object.class");
        assert_eq!((r.module_mutf8(), r.resource_mutf8()), (b"java.base".as_slice(), b"java/lang/Object.class".as_slice()));
        assert_eq!((r.is_compressed(), r.original_size()), (compressed, 160));
        let mut scratch = vec![0xaa; 19];
        let out = r.payload(&mut scratch).unwrap();
        assert_eq!(out, OBJECT);
        if !compressed { assert_eq!(out.as_ptr(), a.bytes()[span].as_ptr()); }
        assert_eq!(digest(out), "1467daa5037c5ca733b7042b3616630bde7413f75a9c281256ff5873ba25912d");
        for (path, expected) in [(b"java/lang/without_ext".as_slice(), b"I'm file without an extension".as_slice()), (b"without_parent.txt".as_slice(), b"I'm file without a parent".as_slice())] {
            let selected = i.select_resource(b"java.base", path).unwrap().unwrap();
            assert_eq!(selected.payload(&mut scratch).unwrap(), expected);
        }
        for (path, expected) in [(b"java/lang/hank.txt".as_slice(), "2464c7c9126b014901d58b57c86abd0e10695527708ca326178147786ba32824"), (b"module-info.class".as_slice(), "c8a4a968e9bc23901b35efede100d8cb8c93975679ca0adb5b45d2fc6a6db815")] {
            let selected = i.select_resource(b"java.base", path).unwrap().unwrap();
            assert_eq!(digest(selected.payload(&mut scratch).unwrap()), expected);
        }
        assert!(i.select_resource(b"modules", b"java.base/java/lang").unwrap().is_none());
        assert!(i.select_resource(b"packages", b"java.lang").unwrap().is_none());
        assert!(i.select_resource(b"other", b"java/lang/Object.class").unwrap().is_none());
        assert!(i.select_resource(b"java.base", b"absent").unwrap().is_none());
    }
}

#[test]
fn header_counts_empty_and_all_truncations_are_atomic() {
    for n in 1..28 { assert_eq!(rejected(PLAIN_LE[..n].to_vec()), E::Truncated); }
    let empty = image(&[]);
    assert_eq!(index_jimage(&verified(empty.bytes)).unwrap().resource_count(), 0);
    for (at, value, expected) in [
        (0, 0, E::InvalidMagic), (4, 0x10001, E::UnsupportedVersion(0x10001)),
        (8, 1, E::UnsupportedFlags(1)), (12, 13, E::Malformed("resource count")),
        (12, 11, E::Malformed("resource count")), (16, u32::MAX, E::Limit("table slots")),
        (20, u32::MAX, E::Limit("index bytes")), (24, u32::MAX, E::Limit("index bytes")),
    ] {
        let mut b = PLAIN_LE.to_vec(); put32(&mut b, at, value); assert_eq!(rejected(b), expected);
    }
    for n in 28..424 { assert!(matches!(rejected(PLAIN_LE[..n].to_vec()), E::Truncated)); }
    let mut b = PLAIN_LE.to_vec(); b.truncate(7526); assert!(matches!(rejected(b), E::Malformed("resource span")));
}

#[test]
fn perfect_hash_redirects_cannot_authorize_false_absence_or_positives() {
    for value in [i32::MIN as u32, (-14i32) as u32] {
        let mut b = PLAIN_LE.to_vec(); put32(&mut b, 28, value); assert_eq!(rejected(b), E::Malformed("redirect slot"));
    }
    // Every bucket mutation is validated against every retained identity before lookup.
    for bucket in 0..12 {
        let mut b = PLAIN_LE.to_vec(); put32(&mut b, 28 + 4 * bucket, 0);
        if PLAIN_LE[28 + 4 * bucket..32 + 4 * bucket] != [0, 0, 0, 0] {
            assert_eq!(rejected(b), E::Malformed("hash consistency"));
        }
    }
    let a = verified(PLAIN_LE.to_vec()); let i = index_jimage(&a).unwrap();
    // A valid table's candidate slot is not sufficient: complete identity is checked.
    for n in 0..300 { assert!(i.select_resource(b"java.base", format!("unknown-{n}").as_bytes()).unwrap().is_none()); }
    let mut authored = image(&[Record::stored(b"m", b"x", b"a")]);
    let unused = (0..7).find(|&n| n != authored.slots[0]).unwrap();
    put32(&mut authored.bytes, 28, (-1 - unused as i32) as u32);
    assert_eq!(rejected(authored.bytes), E::Malformed("redirect slot"));
}

#[test]
fn complete_locations_reject_midstream_aliases_truncation_repetition_and_unknowns() {
    let original = image(&[Record::stored(b"m", b"x", b"data")]);
    for (relative, value) in [(0, 0xff), (0, 1), (9, 15), (63, 15)] {
        let mut b = original.bytes.clone(); b[original.locations[0] + relative] = value;
        assert!(matches!(rejected(b), E::Malformed(_)));
    }
    let mut b = original.bytes.clone();
    // Point to the middle of the first eight-byte value; no unseen prefix is allowed.
    put32(&mut b, 28 + 4 * 7 + 4 * original.slots[0], 2);
    assert!(matches!(rejected(b), E::Malformed(_)));
    let mut b = original.bytes.clone();
    put32(&mut b, 28 + 4 * 7 + 4 * original.slots[0], u32::MAX);
    assert_eq!(rejected(b), E::Malformed("location stream"));
    let two = image(&[Record::stored(b"m", b"x", b"a"), Record::stored(b"m", b"y", b"b")]);
    let mut b = two.bytes.clone();
    put32(&mut b, 28 + 4 * 11 + 4 * two.slots[1], 1);
    assert!(matches!(rejected(b), E::Malformed("hash consistency") | E::Malformed("overlapping locations")));
    let mut b = original.bytes.clone(); b[original.locations[0] + 63] = 7;
    assert_eq!(rejected(b), E::Malformed("location attribute"));
}

#[test]
fn mutf8_is_lossless_and_string_boundaries_are_global() {
    for path in [b"p/\xed\xa0\xbd\xed\xb8\x80".as_slice(), b"p/\xed\xa0\x80", b"p/\xc0\x80", b"p/../%2e"] {
        let a = verified(image(&[Record::stored(b"m", path, b"original")]).bytes);
        let i = index_jimage(&a).unwrap(); let r = i.select_resource(b"m", path).unwrap().unwrap();
        assert_eq!(r.resource_mutf8(), path); assert_eq!(r.payload(&mut Vec::new()).unwrap(), b"original");
        assert!(i.select_resource(b"m", b"p/normalized").unwrap().is_none());
    }
    for invalid in [b"\xf0\x9f\x98\x80".as_slice(), b"\xc0\xaf", b"\xc1\x81", b"\xe0\x80\x80", b"\xe1\x80", b"\x80"] {
        let a = verified(image(&[Record::stored(b"m", b"valid", b"x")]).bytes); let i = index_jimage(&a).unwrap();
        assert!(i.select_resource(b"m", invalid).is_err());
        assert!(i.select_resource(b"m", b"a\0b").is_err()); // raw NUL is a table separator, not an encoded name unit
        assert!(index_jimage(&verified(image(&[Record::stored(b"m", invalid, b"x")]).bytes)).is_err());
    }
    let original = image(&[Record::stored(b"m", b"valid", b"x")]);
    let mut b = original.bytes.clone();
    attr(&mut b, original.locations[0], 3, 2); // middle of "zip"
    assert_eq!(rejected(b), E::Malformed("string boundary"));
    let mut b = original.bytes.clone(); b[original.data_start - 1] = b'x';
    assert_eq!(rejected(b), E::Malformed("unterminated string"));
    let mut b = original.bytes; b[original.strings_start + 16] = 0xff; // unreferenced plugin string still validated
    assert_eq!(rejected(b), E::Malformed("MUTF-8"));
}

#[test]
fn module_identity_and_internal_records_are_not_global_roots() {
    let mut internal = Record::stored(b"modules", b"ignored", b"metadata");
    internal.base = b"m/p".to_vec();
    let a = verified(image(&[Record::stored(b"one", b"p/C.class", b"one"), Record::stored(b"two", b"p/C.class", b"two"), internal]).bytes);
    let i = index_jimage(&a).unwrap(); assert_eq!(i.resource_count(), 3);
    for module in [b"one".as_slice(), b"two"] {
        let r = i.select_resource(module, b"p/C.class").unwrap().unwrap();
        assert_eq!(r.payload(&mut Vec::new()).unwrap(), module);
    }
    assert!(i.select_resource(b"modules", b"m/p").unwrap().is_none());
    assert!(i.select_resource(b"", b"p/C.class").is_err());
    assert!(i.select_resource(b"one/two", b"p/C.class").is_err());
    // Distinct attribute decompositions can spell the same complete identity.
    let source = image(&[Record::stored(b"m", b"a/b", b"one"), Record::stored(b"m", b"a/c", b"two")]);
    let mut b = source.bytes;
    let first_base = u64::from_be_bytes(b[source.locations[0] + 19..source.locations[0] + 27].try_into().unwrap());
    attr(&mut b, source.locations[1], 3, first_base);
    assert!(matches!(rejected(b), E::Malformed("hash consistency") | E::Malformed("duplicate identity")));
}

#[test]
fn original_resource_spans_check_overflow_overlap_and_zero_length() {
    let original = image(&[Record::stored(b"m", b"x", b"abc"), Record::stored(b"m", b"y", b"def")]);
    for (kind, value) in [(5, u64::MAX), (7, u64::MAX), (6, u64::MAX)] {
        let mut b = original.bytes.clone(); attr(&mut b, original.locations[0], kind, value);
        assert_eq!(rejected(b), E::Malformed("resource span"));
    }
    let mut b = original.bytes.clone(); attr(&mut b, original.locations[1], 5, 2);
    assert_eq!(rejected(b), E::Malformed("overlapping resources"));
    let zero = image(&[Record::stored(b"m", b"x", b"abc"), Record::stored(b"m", b"empty", b"")]);
    let a = verified(zero.bytes); let i = index_jimage(&a).unwrap();
    let r = i.select_resource(b"m", b"empty").unwrap().unwrap();
    assert_eq!(r.stored_span(), a.bytes().len()..a.bytes().len());
    let mut scratch = vec![9; 3]; assert!(r.payload(&mut scratch).unwrap().is_empty()); assert!(scratch.is_empty());
}

#[test]
fn compressed_headers_wrapper_adler_consumption_and_sizes_fail_closed() {
    let valid = zip_layer(b"original", 1, 1);
    for n in 1..29 { assert!(matches!(payload_error(compressed(valid[..n].to_vec(), 8)), E::Malformed(_) | E::SizeMismatch)); }
    for (at, value, expected) in [(0, 0, E::Malformed("compressed magic")), (28, 2, E::Malformed("terminal byte")), (24, 0, E::UnsupportedCompression)] {
        let mut b = valid.clone(); b[at] = value; assert_eq!(payload_error(compressed(b, 8)), expected);
    }
    for plugin in [5, 16] { let mut b = valid.clone(); put32(&mut b, 20, plugin); assert_eq!(payload_error(compressed(b, 8)), E::UnsupportedCompression); }
    let mut b = valid.clone(); put32(&mut b, 20, 2); assert_eq!(payload_error(compressed(b, 8)), E::Malformed("string boundary"));
    let mut b = valid.clone(); b[4..12].copy_from_slice(&u64::MAX.to_le_bytes()); assert!(matches!(payload_error(compressed(b, 8)), E::Limit(_)));
    for size in [0u64, 7, 9] {
        let mut b = valid.clone(); b[12..20].copy_from_slice(&size.to_le_bytes());
        assert!(matches!(payload_error(compressed(b, 8)), E::SizeMismatch | E::InvalidCompressedStream));
    }
    let mut b = valid.clone(); *b.last_mut().unwrap() ^= 1; assert_eq!(payload_error(compressed(b, 8)), E::InvalidCompressedStream);
    // Dictionary request, malformed CMF/FLG, and raw DEFLATE are never ZIP fallbacks.
    for header in [[0x78, 0x20], [0x79, 0x01], [0x01, 0x08]] {
        let mut b = valid.clone(); b[29..31].copy_from_slice(&header); assert_eq!(payload_error(compressed(b, 8)), E::InvalidCompressedStream);
    }
    let mut b = valid.clone(); b.push(0); let size = (b.len() - 29) as u64; b[4..12].copy_from_slice(&size.to_le_bytes());
    assert_eq!(payload_error(compressed(b, 8)), E::TrailingCompressedInput);
    let mut b = valid.clone(); b.pop(); let size = (b.len() - 29) as u64; b[4..12].copy_from_slice(&size.to_le_bytes());
    assert_eq!(payload_error(compressed(b, 8)), E::InvalidCompressedStream);
    let mut raw_deflate = valid[..29].to_vec();
    raw_deflate.extend_from_slice(&valid[31..valid.len() - 4]);
    let size = (raw_deflate.len() - 29) as u64;
    raw_deflate[4..12].copy_from_slice(&size.to_le_bytes());
    assert_eq!(payload_error(compressed(raw_deflate, 8)), E::InvalidCompressedStream);
    assert_eq!(payload_error(compressed(valid, 9)), E::SizeMismatch);
    // Stored compressed_size=0 is NEVER header-probed, even with a valid envelope.
    let raw = zip_layer(b"original", 1, 1);
    let a = verified(image(&[Record::stored(b"m", b"raw", &raw)]).bytes); let i = index_jimage(&a).unwrap();
    assert_eq!(i.select_resource(b"m", b"raw").unwrap().unwrap().payload(&mut Vec::new()).unwrap(), raw);
}

#[test]
fn nested_zip_loop_ignores_terminal_control_and_unsupported_inner_is_atomic() {
    for terminal in [0, 1] {
        let inner = zip_layer(b"original", terminal, 1);
        let a = verified(image(&[compressed(zip_layer(&inner, terminal, 1), 8)]).bytes);
        let i = index_jimage(&a).unwrap(); let r = i.select_resource(b"m", b"p/resource").unwrap().unwrap();
        assert_eq!(r.payload(&mut Vec::new()).unwrap(), b"original");
    }
    let unsupported = zip_layer(b"transformed", 1, 5);
    assert_eq!(payload_error(compressed(zip_layer(&unsupported, 1, 1), 11)), E::UnsupportedCompression);
    let mut data = b"original".to_vec();
    for _ in 0..8 { data = zip_layer(&data, 0, 1); }
    let a = verified(image(&[compressed(data.clone(), 8)]).bytes); let i = index_jimage(&a).unwrap();
    assert_eq!(i.select_resource(b"m", b"p/resource").unwrap().unwrap().payload(&mut Vec::new()).unwrap(), b"original");
    assert_eq!(payload_error(compressed(zip_layer(&data, 1, 1), 8)), E::Limit("compression layers"));
    let empty = compressed(zip_layer(b"", 1, 1), 0);
    let a = verified(image(&[empty]).bytes); let i = index_jimage(&a).unwrap();
    assert!(i.select_resource(b"m", b"p/resource").unwrap().unwrap().payload(&mut Vec::new()).unwrap().is_empty());
}

#[test]
fn exact_payload_identity_and_late_global_capacity_work_limits() {
    for length in [MAX, MAX + 1] {
        let data = vec![0; length];
        let a = verified(image(&[Record::stored(b"m", b"raw", &data)]).bytes); let i = index_jimage(&a).unwrap();
        let r = i.select_resource(b"m", b"raw").unwrap().unwrap(); let mut scratch = vec![1; 8];
        if length == MAX { assert_eq!(r.payload(&mut scratch).unwrap().len(), MAX); }
        else { assert_eq!(r.payload(&mut scratch), Err(E::Limit("selected stored bytes"))); assert!(scratch.is_empty()); }
    }
    for length in [4093, 4094] {
        let name = vec![b'a'; length];
        let a = verified(image(&[Record::stored(b"m", &name, b"x")]).bytes);
        if length == 4093 { let i = index_jimage(&a).unwrap(); assert!(i.select_resource(b"m", &name).unwrap().is_some()); }
        else { assert_eq!(index_jimage(&a).unwrap_err(), E::Limit("identity bytes")); }
    }
    // Output cap exact/plus-one without a large compressed carrier.
    let raw = include_bytes!("../../ktlsp-binary/fixtures/zip-zero-8388608.deflate.bin");
    let mut exact = zip_layer(b"", 1, 1);
    exact.truncate(29);
    exact[4..12].copy_from_slice(&((raw.len() + 6) as u64).to_le_bytes());
    exact[12..20].copy_from_slice(&(MAX as u64).to_le_bytes());
    exact.extend([0x78, 0x01]); exact.extend(raw);
    exact.extend(((((MAX % 65_521) as u32) << 16) | 1).to_be_bytes());
    let a = verified(image(&[compressed(exact, MAX)]).bytes); let i = index_jimage(&a).unwrap();
    let r = i.select_resource(b"m", b"p/resource").unwrap().unwrap();
    let mut scratch = Vec::new();
    let out = r.payload(&mut scratch).unwrap(); assert_eq!(out.len(), MAX); assert!(out.iter().all(|&v| v == 0));
    let mut b = zip_layer(b"x", 1, 1); b[12..20].copy_from_slice(&((MAX + 1) as u64).to_le_bytes());
    assert_eq!(payload_error(compressed(b, 1)), E::Limit("decoded layer bytes"));
    let mut large = zip_layer(&vec![0; 6 * 1024 * 1024], 1, 1);
    large = zip_layer(&large, 1, 1);
    // Both layers fit the selected caps independently; their combined work does not.
    assert_eq!(payload_error(compressed(large, 6 * 1024 * 1024)), E::Limit("decompression work"));
    let a = verified(image(&[compressed(zip_layer(b"ok", 1, 1), 2)]).bytes); let i = index_jimage(&a).unwrap();
    let r = i.select_resource(b"m", b"p/resource").unwrap().unwrap();
    let mut scratch = Vec::with_capacity(2 * MAX + 1); scratch.push(9);
    assert_eq!(r.payload(&mut scratch), Err(E::Limit("live decode capacity"))); assert!(scratch.is_empty());
    // A supplied exact aggregate capacity works for a one-layer result, but
    // cannot be independently re-granted to an inner output buffer.
    let a = verified(image(&[compressed(zip_layer(b"ok", 1, 1), 2)]).bytes); let i = index_jimage(&a).unwrap();
    let r = i.select_resource(b"m", b"p/resource").unwrap().unwrap();
    let mut scratch = Vec::with_capacity(2 * MAX);
    assert_eq!(r.payload(&mut scratch).unwrap(), b"ok");
    let a = verified(image(&[compressed(zip_layer(&zip_layer(b"ok", 1, 1), 1, 1), 2)]).bytes); let i = index_jimage(&a).unwrap();
    let r = i.select_resource(b"m", b"p/resource").unwrap().unwrap();
    assert_eq!(r.payload(&mut scratch), Err(E::Limit("live decode capacity"))); assert!(scratch.is_empty());
}

#[test]
fn complete_index_slot_string_capacity_and_late_shared_prefix_work_limits() {
    for slots in [0usize, 16_384, 16_385] {
        let mut b = Vec::new();
        for v in [0xcafedadau32, 0x10000, 0, 0, slots as u32, 1, 1] { b.extend(v.to_le_bytes()); }
        b.resize(28 + 8 * slots + 2, 0);
        let a = verified(b);
        if slots <= 16_384 {
            let i = index_jimage(&a).unwrap(); assert_eq!(i.resource_count(), 0);
            assert!(i.select_resource(b"m", b"absent").unwrap().is_none());
        } else { assert_eq!(index_jimage(&a).unwrap_err(), E::Limit("table slots")); }
    }
    for strings in [65_536usize, 65_537] {
        let original = image(&[Record::stored(b"m", b"x", b"data")]);
        let old_count = original.bytes[original.strings_start..original.data_start].iter().filter(|&&v| v == 0).count();
        let extra = strings - old_count;
        let mut b = original.bytes;
        b.splice(original.data_start..original.data_start, std::iter::repeat_n(0, extra));
        put32(&mut b, 24, (original.data_start - original.strings_start + extra) as u32);
        let a = verified(b);
        if strings == 65_536 {
            let i = index_jimage(&a).unwrap();
            assert_eq!(i.select_resource(b"m", b"x").unwrap().unwrap().payload(&mut Vec::new()).unwrap(), b"data");
        } else { assert_eq!(index_jimage(&a).unwrap_err(), E::Limit("strings")); }
    }
    // A single interned long parent is scanned once; full identity emission and
    // comparisons still share the original operation's work/storage ceilings.
    for count in [100usize, 400, 600] {
        let records: Vec<_> = (0..count).map(|n| {
            let mut r = Record::stored(b"m", format!("entry-{n:04}").as_bytes(), b"x");
            r.parent = vec![b'p'; 3980]; r
        }).collect();
        let a = verified(image(&records).bytes);
        if count == 100 {
            let i = index_jimage(&a).unwrap();
            let mut path = vec![b'p'; 3980]; path.extend(b"/entry-0099");
            assert_eq!(i.select_resource(b"m", &path).unwrap().unwrap().payload(&mut Vec::new()).unwrap(), b"x");
        } else {
            let expected = if count == 400 { E::Limit("index work") } else { E::Limit("metadata capacity") };
            assert_eq!(index_jimage(&a).unwrap_err(), expected);
        }
    }
}
