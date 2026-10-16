use ktlsp_binary::{Constant, Error, MAX_BYTES, MAX_RECORDS, parse_class};

// Independently authored binary data: no compiler or JVM generated these fixtures.
fn u2(b: &mut Vec<u8>, n: u16) {
    b.extend(n.to_be_bytes());
}
fn u4(b: &mut Vec<u8>, n: u32) {
    b.extend(n.to_be_bytes());
}
#[derive(Default)]
struct Pool {
    bytes: Vec<u8>,
    slots: u16,
}
impl Pool {
    fn entry(&mut self, tag: u8, data: &[u8]) -> u16 {
        let index = self.slots + 1;
        self.bytes.push(tag);
        self.bytes.extend(data);
        self.slots += if tag == 5 || tag == 6 { 2 } else { 1 };
        index
    }
    fn text(&mut self, s: &[u8]) -> u16 {
        let mut b = Vec::new();
        u2(&mut b, s.len() as u16);
        b.extend(s);
        self.entry(1, &b)
    }
    fn one(&mut self, tag: u8, i: u16) -> u16 {
        self.entry(tag, &i.to_be_bytes())
    }
    fn two(&mut self, tag: u8, a: u16, b: u16) -> u16 {
        let mut v = Vec::new();
        u2(&mut v, a);
        u2(&mut v, b);
        self.entry(tag, &v)
    }
    fn base() -> Self {
        let mut p = Self::default();
        let a = p.text(b"Sample");
        p.one(7, a);
        let b = p.text(b"java/lang/Object");
        p.one(7, b);
        p
    }
    fn file(self, tail: &[u8]) -> Vec<u8> {
        let mut b = Vec::new();
        u4(&mut b, 0xcafebabe);
        u2(&mut b, 0);
        u2(&mut b, 67);
        u2(&mut b, self.slots + 1);
        b.extend(self.bytes);
        u2(&mut b, 0x21);
        u2(&mut b, 2);
        u2(&mut b, 4);
        b.extend(tail);
        b
    }
}
fn empty() -> Vec<u8> {
    Pool::base().file(&[0; 8])
}
fn member(b: &mut Vec<u8>, flags: u16, name: u16, desc: u16, attrs: u16) {
    for v in [flags, name, desc, attrs] {
        u2(b, v);
    }
}
fn member_file(desc: &[u8], method: bool, flags: u16, name: &[u8]) -> Vec<u8> {
    let mut p = Pool::base();
    let n = p.text(name);
    let d = p.text(desc);
    let mut t = Vec::new();
    u2(&mut t, 0);
    u2(&mut t, u16::from(!method));
    if !method {
        member(&mut t, flags, n, d, 0);
    }
    u2(&mut t, u16::from(method));
    if method {
        member(&mut t, flags, n, d, 0);
    }
    u2(&mut t, 0);
    p.file(&t)
}
#[test]
fn reads_members_interfaces_and_borrowed_attributes() {
    let mut p = Pool::base();
    let f = p.text(b"field");
    let fd = p.text(b"[[Ljava/lang/String;");
    let m = p.text(b"<init>");
    let md = p.text(b"(J[DLpkg/Thing;)V");
    let a = p.text(b"Unknown");
    let mut t = Vec::new();
    u2(&mut t, 1);
    u2(&mut t, 4);
    u2(&mut t, 1);
    member(&mut t, 1, f, fd, 1);
    u2(&mut t, a);
    u4(&mut t, 3);
    t.extend([0, 255, 1]);
    u2(&mut t, 1);
    member(&mut t, 1, m, md, 0);
    u2(&mut t, 1);
    u2(&mut t, a);
    u4(&mut t, 0);
    let bytes = p.file(&t);
    let c = parse_class(&bytes).unwrap();
    assert_eq!(c.interfaces, [4]);
    assert_eq!(c.fields[0].access_flags, 1);
    assert_eq!(c.fields[0].attributes[0].bytes, [0, 255, 1]);
    assert_eq!(c.methods[0].name, m);
    assert!(c.attributes[0].bytes.is_empty());
    assert_eq!(
        c.utf16(fd).unwrap(),
        "[[Ljava/lang/String;".encode_utf16().collect::<Vec<_>>()
    );
    let attr = c.fields[0].attributes[0].bytes;
    assert!(
        attr.as_ptr() >= bytes.as_ptr() && attr.as_ptr() < bytes.as_ptr().wrapping_add(bytes.len())
    );
    assert!(c.utf16(0).is_err());
    assert!(c.utf16(2).is_err());
}
#[test]
fn all_tags_wide_slots_and_lossless_modified_utf8() {
    let mut p = Pool::base();
    for tag in [3, 4] {
        p.entry(tag, &[0xff; 4]);
    }
    let long = p.entry(5, &[0x81; 8]);
    let double = p.entry(6, &[0xff; 8]);
    let s = p.text(&[0xc0, 0x80, 0xed, 0xa0, 0x80, 0xed, 0xb0, 0x80, 0x7f]);
    p.one(8, s);
    p.one(19, 1);
    p.one(20, 1);
    let n = p.text(b"f");
    let d = p.text(b"I");
    let nat = p.two(12, n, d);
    let field = p.two(9, 2, nat);
    for kind in 1..=4 {
        let mut b = vec![kind];
        u2(&mut b, field);
        p.entry(15, &b);
    }
    p.two(17, 65535, nat);
    let md = p.text(b"()V");
    let mn = p.two(12, n, md);
    let method = p.two(10, 2, mn);
    let interface = p.two(11, 2, mn);
    for (kind, target) in [(5, method), (6, interface), (7, method), (9, interface)] {
        let mut b = vec![kind];
        u2(&mut b, target);
        p.entry(15, &b);
    }
    let init = p.text(b"<init>");
    let initnat = p.two(12, init, md);
    let initref = p.two(10, 2, initnat);
    let mut b = vec![8];
    u2(&mut b, initref);
    p.entry(15, &b);
    p.one(16, md);
    p.two(18, 65535, mn);
    let b = p.file(&[0; 8]);
    let c = parse_class(&b).unwrap();
    for end in 0..b.len() {
        assert!(parse_class(&b[..end]).is_err());
    }
    assert_eq!(c.constants[long as usize + 1], Constant::Unusable);
    assert_eq!(c.constants[double as usize + 1], Constant::Unusable);
    assert_eq!(c.utf16(s).unwrap(), [0, 0xd800, 0xdc00, 127]);
}
#[test]
fn every_byte_truncation_and_trailing_data() {
    for b in [
        empty(),
        member_file(b"(JLjava/lang/Object;)V", true, 1, b"f"),
    ] {
        for end in 0..b.len() {
            assert!(parse_class(&b[..end]).is_err(), "prefix {end}");
        }
        let mut extra = b;
        extra.push(0);
        assert_eq!(
            parse_class(&extra).unwrap_err(),
            Error::Malformed("trailing bytes")
        );
    }
}
#[test]
fn version_policy_and_tag_version_gates() {
    for (major, minor, ok) in [
        (44u16, 0u16, false),
        (45, 0, true),
        (45, 1, true),
        (45, 2, true),
        (45, 3, true),
        (45, 4, false),
        (46, 0, true),
        (46, 1, false),
        (55, 0, true),
        (55, 1, false),
        (67, 0, true),
        (67, 65535, false),
        (68, 0, true),
        (69, 0, true),
        (68, 1, false),
        (69, 1, false),
        (68, 65535, false),
        (69, 65535, false),
        (70, 0, false),
        (u16::MAX, u16::MAX, false),
    ] {
        let mut b = empty();
        b[4..6].copy_from_slice(&minor.to_be_bytes());
        b[6..8].copy_from_slice(&major.to_be_bytes());
        assert_eq!(parse_class(&b).is_ok(), ok, "{major}.{minor}");
    }
    for (tag, major) in [
        (15, 50u16),
        (16, 50),
        (18, 50),
        (19, 52),
        (20, 52),
        (17, 54),
    ] {
        let mut p = Pool::base();
        p.entry(tag, &[0; 4]);
        let mut b = p.file(&[0; 8]);
        b[6..8].copy_from_slice(&major.to_be_bytes());
        assert_eq!(
            parse_class(&b).unwrap_err(),
            Error::Malformed("constant tag version")
        );
    }
}
#[test]
fn malformed_tags_indices_wide_slots_utf8_and_lengths() {
    for tag in [0, 2, 13, 14, 21, 255] {
        let mut p = Pool::base();
        p.entry(tag, &[]);
        assert_eq!(
            parse_class(&p.file(&[0; 8])).unwrap_err(),
            Error::UnsupportedTag(tag)
        );
    }
    for (tag, target) in [(7, 0), (7, 2), (8, 65535), (16, 2), (19, 2), (20, 2)] {
        let mut p = Pool::base();
        p.one(tag, target);
        assert!(parse_class(&p.file(&[0; 8])).is_err());
    }
    for tag in [9, 10, 11, 12, 17, 18] {
        let mut p = Pool::base();
        p.two(tag, 2, 2);
        assert!(parse_class(&p.file(&[0; 8])).is_err());
    }
    let mut p = Pool::base();
    p.entry(5, &[0; 8]);
    p.slots -= 1;
    assert_eq!(
        parse_class(&p.file(&[0; 8])).unwrap_err(),
        Error::Malformed("wide constant slot")
    );
    for text in [
        vec![0],
        vec![0xc0, 0x81],
        vec![0xc1, 0xbf],
        vec![0xc2],
        vec![0xc2, 0x7f],
        vec![0xe0, 0x80, 0x80],
        vec![0xed, 0xa0],
        vec![0xf0, 0x90, 0x80, 0x80],
        vec![0xff],
    ] {
        let mut p = Pool::base();
        p.text(&text);
        assert_eq!(
            parse_class(&p.file(&[0; 8])).unwrap_err(),
            Error::Malformed("modified UTF8")
        );
    }
    for len in [1, u32::MAX] {
        let mut t = vec![0; 6];
        u2(&mut t, 1);
        u2(&mut t, 1);
        u4(&mut t, len);
        assert_eq!(
            parse_class(&Pool::base().file(&t)).unwrap_err(),
            Error::Truncated
        );
    }
    let mut b = empty();
    b[8..10].copy_from_slice(&0u16.to_be_bytes());
    assert!(parse_class(&b).is_err());
}
#[test]
fn descriptors_names_and_slot_boundaries() {
    for d in [
        b"".as_slice(),
        b"V",
        b"Igarbage",
        b"[V",
        b"L;",
        b"La//b;",
        b"La.b;",
        b"Labc",
        b"(I)V",
    ] {
        assert!(
            parse_class(&member_file(d, false, 1, b"f")).is_err(),
            "{d:?}"
        );
    }
    for d in [
        b"I".as_slice(),
        b"()",
        b"(V)V",
        b"(I",
        b"()VI",
        b"()L;",
        b"([)V",
    ] {
        assert!(
            parse_class(&member_file(d, true, 1, b"f")).is_err(),
            "{d:?}"
        );
    }
    let mut d = vec![b'['; 255];
    d.push(b'I');
    assert!(parse_class(&member_file(&d, false, 1, b"f")).is_ok());
    d.insert(0, b'[');
    assert!(parse_class(&member_file(&d, false, 1, b"f")).is_err());
    for (count, flags, ok) in [
        (254, 1, true),
        (255, 1, false),
        (255, 9, true),
        (256, 9, false),
    ] {
        let mut d = vec![b'('];
        d.extend(vec![b'I'; count]);
        d.extend(b")V");
        assert_eq!(parse_class(&member_file(&d, true, flags, b"f")).is_ok(), ok);
    }
    let mut d = vec![b'('];
    d.extend(vec![b'J'; 128]);
    d.extend(b")V");
    assert!(parse_class(&member_file(&d, true, 9, b"f")).is_err());
    for (name, desc, flags, ok) in [
        (b"<init>".as_slice(), b"()V".as_slice(), 1, true),
        (b"<init>", b"()I", 1, false),
        (b"<clinit>", b"()V", 8, true),
        (b"<clinit>", b"()V", 1, false),
        (b"<bad>", b"()V", 1, false),
        (b"a/b", b"()V", 1, false),
        (b"", b"()V", 1, false),
    ] {
        assert_eq!(
            parse_class(&member_file(desc, true, flags, name)).is_ok(),
            ok
        );
    }
}
#[test]
fn handle_kinds_and_constructor_targets() {
    for kind in 0..=10 {
        let mut p = Pool::base();
        let n = p.text(b"f");
        let d = p.text(b"()V");
        let nat = p.two(12, n, d);
        let target = p.two(10, 2, nat);
        let mut v = vec![kind];
        u2(&mut v, target);
        p.entry(15, &v);
        assert_eq!(
            parse_class(&p.file(&[0; 8])).is_ok(),
            [5, 6, 7].contains(&kind),
            "kind {kind}"
        );
    }
}
#[test]
fn rejects_resource_budgets_and_repeated_reference_work() {
    assert_eq!(
        parse_class(&vec![0; MAX_BYTES + 1]).unwrap_err(),
        Error::Limit("bytes")
    );
    let mut t = Vec::new();
    u2(&mut t, MAX_RECORDS as u16);
    assert_eq!(
        parse_class(&Pool::base().file(&t)).unwrap_err(),
        Error::Limit("records")
    );
    let mut p = Pool::base();
    let name = p.text(&vec![b'a'; 65535]);
    let desc = p.text(b"I");
    for _ in 0..1100 {
        p.two(12, name, desc);
    }
    assert_eq!(
        parse_class(&p.file(&[0; 8])).unwrap_err(),
        Error::Limit("validation work")
    );
}
#[test]
fn deterministic_mutation_and_random_bytes_never_panic() {
    let b = member_file(b"(J[I)Ljava/lang/Object;", true, 1, b"f");
    for i in 0..b.len() {
        for value in 0..=255 {
            let mut m = b.clone();
            m[i] = value;
            let _ = parse_class(&m);
        }
    }
    let mut seed = 0x12345678u32;
    for len in 0..1024 {
        let mut b = vec![0; len];
        for c in &mut b {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            *c = seed as u8;
        }
        let _ = parse_class(&b);
    }
}

#[test]
fn historical_class_initializers() {
    for major in [45u16, 50, 51, 67, 68, 69] {
        let mut b = member_file(b"(I)V", true, 0, b"<clinit>");
        b[6..8].copy_from_slice(&major.to_be_bytes());
        assert_eq!(parse_class(&b).is_ok(), major < 51);
    }
}

#[test]
fn unused_name_and_type_rejects_class_initializer_method_name() {
    for (descriptor, ok) in [(b"()V".as_slice(), false), (b"I".as_slice(), true)] {
        let mut p = Pool::base();
        let name = p.text(b"<clinit>");
        let desc = p.text(descriptor);
        p.two(12, name, desc);
        assert_eq!(parse_class(&p.file(&[0; 8])).is_ok(), ok);
    }
}

// All version/header variations here are authored controls, never genuine fixtures.
fn at_version(mut bytes: Vec<u8>, major: u16) -> Vec<u8> {
    bytes[6..8].copy_from_slice(&major.to_be_bytes());
    bytes
}

#[test]
fn modern_late_framing_opaque_attributes_and_resource_boundaries() {
    for major in [68, 69] {
        let mut pool = Pool::base();
        let name = pool.text(b"m");
        let desc = pool.text(b"()V");
        let opaque = pool.text(b"Record");
        let mut tail = vec![0; 4]; // no interfaces or fields
        u2(&mut tail, 2);
        member(&mut tail, 1, name, desc, 0);
        member(&mut tail, 9, name, desc, 1);
        u2(&mut tail, opaque);
        u4(&mut tail, 3);
        tail.extend([0xff, 0, 0xff]); // opaque, not a valid Record interior
        u2(&mut tail, 0);
        let bytes = at_version(pool.file(&tail), major);
        let class = parse_class(&bytes).unwrap();
        assert_eq!(class.methods[1].attributes[0].bytes, [0xff, 0, 0xff]);
        let attr = class.methods[1].attributes[0].bytes;
        assert_eq!(attr.as_ptr(), bytes[bytes.len() - 5..].as_ptr());
        // Includes the final member, its attribute header/payload and class attrs.
        for end in bytes.len() - 19..bytes.len() {
            assert_eq!(parse_class(&bytes[..end]).unwrap_err(), Error::Truncated, "{major}: {end}");
        }
        let mut huge = bytes.clone();
        let at = huge.len() - 9;
        huge[at..at + 4].copy_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(parse_class(&huge).unwrap_err(), Error::Truncated);
        let mut extra = bytes.clone();
        extra.push(0);
        assert_eq!(parse_class(&extra).unwrap_err(), Error::Malformed("trailing bytes"));
        let mut over = bytes;
        over.resize(MAX_BYTES + 1, 0);
        assert_eq!(parse_class(&over).unwrap_err(), Error::Limit("bytes"));
        let mut records = Vec::new();
        u2(&mut records, MAX_RECORDS as u16);
        assert_eq!(parse_class(&at_version(Pool::base().file(&records), major)).unwrap_err(), Error::Limit("records"));
        let mut pool = Pool::base();
        let name = pool.text(&vec![b'a'; 65535]);
        let desc = pool.text(b"I");
        for _ in 0..1100 { pool.two(12, name, desc); }
        assert_eq!(parse_class(&at_version(pool.file(&[0;8]), major)).unwrap_err(), Error::Limit("validation work"));
    }
}

#[test]
fn modern_constant_pool_and_descriptor_special_name_controls() {
    for major in [68, 69] {
        for tag in [0, 2, 13, 14, 21, 255] {
            let mut pool = Pool::base();
            pool.entry(tag, &[]);
            assert_eq!(parse_class(&at_version(pool.file(&[0;8]), major)).unwrap_err(), Error::UnsupportedTag(tag));
        }
        for (tag, target) in [(7,0),(7,2),(8,0),(16,2),(19,2),(20,2)] {
            let mut pool = Pool::base(); pool.one(tag,target);
            assert!(parse_class(&at_version(pool.file(&[0;8]), major)).is_err());
        }
        for tag in [5,6] {
            let mut pool = Pool::base(); pool.entry(tag,&[0;8]); pool.slots -= 1;
            assert_eq!(parse_class(&at_version(pool.file(&[0;8]), major)).unwrap_err(), Error::Malformed("wide constant slot"));
        }
        for text in [&[0][..], &[0xc0,0x81], &[0xf0,0x90,0x80,0x80]] {
            let mut pool = Pool::base(); pool.text(text);
            assert_eq!(parse_class(&at_version(pool.file(&[0;8]), major)).unwrap_err(), Error::Malformed("modified UTF8"));
        }
        let mut pool = Pool::base();
        let utf = pool.text(&[0xc0,0x80,0xed,0xa0,0x80]);
        let long = pool.entry(5,&[0;8]);
        let bytes = at_version(pool.file(&[0;8]), major);
        let parsed = parse_class(&bytes).unwrap();
        assert_eq!(parsed.utf16(utf).unwrap(), [0,0xd800]);
        assert_eq!(parsed.constants[usize::from(long)+1], Constant::Unusable);
        for (rank, ok) in [(255,true),(256,false)] {
            let desc = [vec![b'[';rank], vec![b'I']].concat();
            assert_eq!(parse_class(&at_version(member_file(&desc,false,1,b"f"),major)).is_ok(),ok);
        }
        for (slots, flags, ok) in [(254,1,true),(255,1,false),(255,9,true),(256,9,false)] {
            let desc = [vec![b'('],vec![b'I';slots],b")V".to_vec()].concat();
            assert_eq!(parse_class(&at_version(member_file(&desc,true,flags,b"m"),major)).is_ok(),ok);
        }
        for (longs, ints, flags, ok) in [(127,0,1,true),(127,1,1,false),(127,1,9,true),(128,0,9,false)] {
            let desc = [vec![b'('],vec![b'J';longs],vec![b'I';ints],b")V".to_vec()].concat();
            assert_eq!(parse_class(&at_version(member_file(&desc,true,flags,b"m"),major)).is_ok(),ok);
        }
        for (name,desc,flags,ok) in [
            (b"<init>".as_slice(),b"()V".as_slice(),1,true),
            (b"<init>",b"()I",1,false),(b"<clinit>",b"()V",8,true),
            (b"<clinit>",b"(I)V",8,false),(b"<clinit>",b"()V",0,false),
            (b"m",b"(Qpkg/C;)V",9,false),(b"m",b"(V)V",9,false),
        ] {
            assert_eq!(parse_class(&at_version(member_file(desc,true,flags,name),major)).is_ok(),ok);
        }
        for (tag, method_name, desc, kind, ok) in [
            (10,b"<init>".as_slice(),b"()V".as_slice(),8,true),
            (10,b"<init>",b"()V",6,false),(10,b"m",b"()V",8,false),
            (11,b"<init>",b"()V",9,false),(10,b"<init>",b"()I",8,false),
            (10,b"<clinit>",b"()V",6,false),(11,b"m",b"()V",6,true),
        ] {
            let mut pool = Pool::base();
            let n = pool.text(method_name); let d = pool.text(desc);
            let nat = pool.two(12,n,d); let target = pool.two(tag,2,nat);
            let mut handle=vec![kind]; u2(&mut handle,target); pool.entry(15,&handle);
            assert_eq!(parse_class(&at_version(pool.file(&[0;8]),major)).is_ok(),ok);
        }
    }
}

#[test]
fn tag_introduction_floors_and_interface_handle_boundary_are_unchanged() {
    for (tag, floor) in [(15,51),(16,51),(18,51),(19,53),(20,53),(17,55)] {
        for major in [floor-1,floor,67,68,69] {
            let mut pool=Pool::base();
            let n=pool.text(b"m"); let d=pool.text(if tag==17 { b"I" } else { b"()V" });
            let nat=pool.two(12,n,d);
            match tag {
                15 => { let target=pool.two(10,2,nat); let mut h=vec![6];u2(&mut h,target);pool.entry(15,&h); }
                16 => { pool.one(16,d); }
                17|18 => { pool.two(tag,0,nat); }
                19|20 => { pool.one(tag,n); }
                _ => unreachable!(),
            }
            let bytes=at_version(pool.file(&[0;8]),major);
            if major<floor { assert_eq!(parse_class(&bytes).unwrap_err(),Error::Malformed("constant tag version")); }
            else { assert!(parse_class(&bytes).is_ok(),"{tag}/{major}"); }
        }
    }
    for major in [51,52,68,69] {
        for kind in [6,7] {
            let mut pool=Pool::base();let n=pool.text(b"m");let d=pool.text(b"()V");let nat=pool.two(12,n,d);
            let target=pool.two(11,2,nat);let mut h=vec![kind];u2(&mut h,target);pool.entry(15,&h);
            assert_eq!(parse_class(&at_version(pool.file(&[0;8]),major)).is_ok(),major>=52);
        }
    }
}
