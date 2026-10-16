use ktlsp_binary::{Attribute, ClassFile, Constant, Member, metadata::linked_class_method::{self, Error, Kind, Limits}};

const PAIR: &[u8] = include_bytes!("fixtures/metadata_constructor/Pair.class.bin");
// Independently authored model annotation: k=1, mv=[2,3,0], direct d1, three d2 strings.
const ATTRIBUTE: &[u8] = &[
    0,1, 0,2, 0,4,
    0,3,b'I',0,4,
    0,5,b'[',0,3,b'I',0,6,b'I',0,7,b'I',0,8,
    0,9,b'[',0,1,b's',0,10,
    0,11,b'[',0,3,b's',0,12,b's',0,13,b's',0,14,
];
fn utf(s: &str) -> Constant { Constant::Utf8(s.encode_utf16().collect()) }
fn model(body: &[u8], constructor: bool) -> ClassFile<'static> {
    // One compact range3 record supplies the three original d2 entries.
    let mut d1 = vec![4,10,2,8,3,24,0,if constructor {66} else {74}];
    assert!(body.len() < 128);
    d1.push(body.len() as u8);
    d1.extend(body);
    let mut encoded = vec![0];
    encoded.extend(d1.iter().map(|byte| u16::from(*byte)));
    let name = if constructor { "<init>" } else { "call" };
    ClassFile {
        minor: 0, major: 52, access_flags: 1, this_class: 15, super_class: 0, interfaces: vec![], fields: vec![],
        constants: vec![Constant::Unusable,utf("RuntimeVisibleAnnotations"),utf("Lkotlin/Metadata;"),utf("k"),Constant::Integer(1),utf("mv"),Constant::Integer(2),Constant::Integer(3),Constant::Integer(0),utf("d1"),Constant::Utf8(encoded),utf("d2"),utf("pkg/Owner.Nested"),utf(name),utf("()V"),Constant::Class(16),utf("pkg/Owner$Nested"),utf(name),utf("()V")],
        methods: vec![Member { access_flags: 1, name: 17, descriptor: 18, attributes: vec![] }],
        attributes: vec![Attribute { name: 1, bytes: ATTRIBUTE }],
    }
}
fn function() -> ClassFile<'static> { model(&[16,1,0xa2,6,2,16,2], false) }
fn constructor() -> ClassFile<'static> { model(&[0xa2,6,2,16,2], true) }
fn add_method(class: &mut ClassFile<'_>, name: &str, desc: &str, flags: u16) {
    let index = class.constants.len() as u16;
    class.constants.extend([utf(name),utf(desc)]);
    class.methods.push(Member { access_flags: flags, name: index, descriptor: index+1, attributes: vec![] });
}
fn result<'c, 'data>(class: &'c ClassFile<'data>) -> Result<linked_class_method::LinkedMethod<'c, 'data>, Error> {
    linked_class_method::decode(class, 1, Limits::default())
}
fn append_d1(class: &mut ClassFile<'_>, bytes: &[u8]) {
    let Constant::Utf8(d1) = &mut class.constants[10] else { panic!() };
    d1.extend(bytes.iter().map(|byte| u16::from(*byte)));
}

#[test]
fn genuine_pair_constructor_and_copy_borrow_original_members_and_descriptors() {
    assert_eq!(PAIR.len(), 2983);
    let class = ktlsp_binary::parse_class(PAIR).unwrap();
    for (ordinal, method_ordinal, cp, span, body_span, extension_span, indices, name, desc, slots) in [
        (5,0,(8,9),76..103,78..103,99..103,(Some(7),Some(8)),"<init>","(Ljava/lang/Object;Ljava/lang/Object;)V",3),
        (9,6,(50,51),149..198,151..198,196..198,(None,Some(18)),"copy","(Ljava/lang/Object;Ljava/lang/Object;)Lkotlin/Pair;",3),
    ] {
        let linked = linked_class_method::decode(&class, ordinal, Limits::default()).unwrap();
        assert!(std::ptr::eq(linked.method, &class.methods[method_ordinal]));
        assert_eq!(linked.method_ordinal, method_ordinal);
        assert_eq!((linked.method.name, linked.method.descriptor), cp);
        assert_eq!(linked.name, name.encode_utf16().collect::<Vec<_>>());
        assert_eq!(linked.name.as_ptr(), class.utf16(cp.0).unwrap().as_ptr());
        assert_eq!(linked.descriptor.raw(), desc.encode_utf16().collect::<Vec<_>>());
        assert_eq!(linked.descriptor.raw().as_ptr(), class.utf16(cp.1).unwrap().as_ptr());
        assert_eq!(linked.descriptor.parameters().count(), 2);
        assert_eq!(linked.descriptor.parameter_slots(), slots);
        assert_eq!(linked.descriptor.result().is_none(), ordinal == 5);
        assert_eq!(linked.member.ordinal, ordinal);
        assert_eq!(linked.member.span, span);
        assert_eq!(linked.member.body_span, body_span.clone());
        assert_eq!(linked.member.extension_bodies, vec![extension_span.clone()]);
        assert_eq!((linked.member.signature.name,linked.member.signature.desc), indices);
        assert_eq!(linked.member_body().unwrap().as_ptr(), linked.decoded_d1()[body_span].as_ptr());
        assert_eq!(&linked.decoded_d1()[extension_span], if ordinal == 5 { &[8,7,16,8][..] } else { &[16,18][..] });
        assert_eq!(linked.class_span, 47..285);
        assert_eq!(linked.class_id.package_name(), "kotlin".encode_utf16().collect::<Vec<_>>());
        assert_eq!(linked.class_id.relative_class_name(), "Pair".encode_utf16().collect::<Vec<_>>());
        assert!(!linked.class_id.local);
        let Constant::Class(owner_index) = class.constants[usize::from(class.this_class)] else { panic!() };
        assert_eq!(linked.jvm_owner.as_ptr(), class.utf16(owner_index).unwrap().as_ptr());
        assert_eq!(linked.jvm_owner, "kotlin/Pair".encode_utf16().collect::<Vec<_>>());
        if ordinal == 9 { assert_eq!(linked.member.function_name, Some(17)); }
    }
    assert_eq!(&PAIR[1925..1931], &[0,1,0,8,0,9]);
}

#[test]
fn name_defaults_and_extension_occurrence_presence_are_not_index_zero() {
    let class = constructor();
    let linked = result(&class).unwrap();
    assert_eq!(linked.member.kind, Kind::Constructor);
    assert_eq!(linked.member.signature.name, None);
    assert_eq!(linked.name, "<init>".encode_utf16().collect::<Vec<_>>());
    let class = function();
    assert_eq!(result(&class).unwrap().member.signature.name, None);
    // Last recognized scalar wins, wrong-wire name/descriptor fields do not clear it;
    // repeated message occurrences merge, including a present empty occurrence.
    let class = model(&[16,1,0xa2,6,4,8,0,16,0,0xa2,6,0,0xa2,6,8,8,1,10,0,16,2,18,0],false);
    let linked = result(&class).unwrap();
    assert_eq!(linked.member.signature.name, Some(1));
    assert_eq!(linked.member.signature.desc, Some(2));
    assert_eq!(linked.member.extension_bodies.len(), 3);
    assert!(linked.member.extension_bodies[1].is_empty());
    let mut class = model(&[16,0,0xa2,6,4,8,1,16,2],false);
    class.constants[13] = utf("jvmName");
    class.constants[17] = utf("jvmName");
    assert_eq!(result(&class).unwrap().member.function_name, Some(0));
    for body in [&[16,1][..], &[16,1,0xa2,6,0], &[16,1,0xa0,6,2], &[16,1,0xa2,6,2,8,1]] {
        assert_eq!(result(&model(body,false)), Err(Error::MissingExplicitDescriptor));
    }
    // A required Function.name is still required even with an explicit JVM name.
    assert!(matches!(result(&model(&[0xa2,6,4,8,1,16,2],false)), Err(Error::Function(_))));
}

#[test]
fn signed_indices_and_full_utf16_identity_do_not_normalize_or_truncate() {
    for bytes in [vec![255,255,255,255,15],vec![128,128,128,128,8]] {
        for tag in [8,16] {
            let mut extension = vec![8,1,16,2,tag];
            extension.extend(&bytes);
            let mut body = vec![16,1,0xa2,6,extension.len() as u8];
            body.extend(extension);
            assert!(matches!(result(&model(&body,false)), Err(Error::Name(_))));
        }
    }
    let mut class = function();
    for index in [13,17] { class.constants[index] = Constant::Utf8(vec![0xd800,97,0xdc00]); }
    assert_eq!(result(&class).unwrap().name, [0xd800,97,0xdc00]);
    class.constants[17] = Constant::Utf8(vec![0xfffd,97,0xfffd]);
    assert_eq!(result(&class), Err(Error::NoMethod));
}

#[test]
fn full_tuple_unique_among_all_original_overloads_and_bridges() {
    let mut class = function();
    add_method(&mut class,"call","(I)V",1);
    add_method(&mut class,"call","()Ljava/lang/Object;",0x1041);
    add_method(&mut class,"other","()V",9);
    assert_eq!(result(&class).unwrap().method_ordinal,0);
    class.methods.swap(0,2);
    assert_eq!(result(&class).unwrap().method_ordinal,2);
    add_method(&mut class,"call","()V",0x1041);
    assert_eq!(result(&class),Err(Error::DuplicateMethod));
    class.methods.pop();
    class.methods.remove(2);
    assert_eq!(result(&class),Err(Error::NoMethod));
}

#[test]
fn actual_flags_slots_constructor_void_and_special_names() {
    let mut class = constructor();
    class.methods[0].access_flags |= 8;
    assert_eq!(result(&class),Err(Error::SpecialMethod));
    class.methods[0].access_flags &= !8;
    for index in [14,18] { class.constants[index] = utf("()I"); }
    assert_eq!(result(&class),Err(Error::SpecialMethod));
    for name in ["<init>","<clinit>"] {
        let mut class = function();
        for index in [13,17] { class.constants[index] = utf(name); }
        assert_eq!(result(&class),Err(Error::SpecialMethod));
    }
    let mut class = function();
    let desc = format!("({})V", "I".repeat(255));
    for index in [14,18] { class.constants[index] = utf(&desc); }
    assert!(matches!(result(&class),Err(Error::Model(ktlsp_binary::Error::Malformed("parameter slots")))));
    class.methods[0].access_flags |= 8;
    assert_eq!(result(&class).unwrap().descriptor.parameter_slots(),255);
    // There are no encoded metadata formals, despite 255 actual JVM parameters.
    assert_eq!(result(&class).unwrap().descriptor.parameters().count(),255);
}

#[test]
fn typed_this_class_owner_mapping_and_no_quoted_local_guesses() {
    let mut class = function();
    assert_eq!(result(&class).unwrap().jvm_owner, "pkg/Owner$Nested".encode_utf16().collect::<Vec<_>>());
    class.this_class = 16;
    assert!(matches!(result(&class),Err(Error::Model(_))));
    class.this_class = 15;
    class.constants[15] = Constant::Class(0);
    assert!(matches!(result(&class),Err(Error::Model(_))));
    class.constants[15] = Constant::Class(16);
    for wrong in ["pkg/Owner/Nested","pkg.Owner$Nested","pkg/Other$Nested"] {
        class.constants[16] = utf(wrong);
        assert!(matches!(result(&class),Err(Error::OwnerMismatch)|Err(Error::Model(_))));
    }
    class.constants[16] = utf("pkg/Owner$Nested");
    class.constants[12] = utf("pkg/`Owner.Nested`");
    assert!(matches!(result(&class),Err(Error::Name(_))));
    class.constants[12] = utf("pkg//Owner.Nested");
    assert!(matches!(result(&class),Err(Error::UnsupportedOwner)));
    let class = model(&[16,1,0xa2,6,2,16,2],false);
    let mut local = class;
    let Constant::Utf8(d1) = &mut local.constants[10] else { panic!() };
    // Extend original StringTableTypes by local_name=0, adjusting its prefix.
    d1[1] = 6;
    d1.splice(6..6,[40,0]);
    assert!(matches!(result(&local),Err(Error::Name(_))));
    let mut root = function();
    root.constants[12] = utf("Owner.Nested"); root.constants[16] = utf("Owner$Nested");
    assert!(result(&root).is_ok());
}

#[test]
fn unsupported_container_versions_wrong_fields_and_late_framing_refuse_atomically() {
    let class = function();
    assert_eq!(linked_class_method::decode(&class,0,Limits::default()),Err(Error::NotMethod));
    assert_eq!(linked_class_method::decode(&class,2,Limits::default()),Err(Error::MissingOrdinal));
    let mut class = function();
    append_d1(&mut class,&[128]);
    assert!(matches!(result(&class),Err(Error::Class(_))|Err(Error::Wire(_))));
    let mut class = function();
    let Constant::Utf8(d1) = &mut class.constants[10] else { panic!() };
    d1[8] = 72; d1[9] = 0; d1.truncate(10);
    assert_eq!(result(&class),Err(Error::NotMethod));
    for (index,value) in [(4,2),(7,4)] {
        let mut class = function(); class.constants[index] = Constant::Integer(value);
        assert_eq!(result(&class),Err(Error::UnsupportedContainer));
    }
    let mut class = function(); class.major = 68;
    assert!(matches!(result(&class),Err(Error::Model(ktlsp_binary::Error::UnsupportedVersion {..}))));
    let mut class = function(); class.attributes.clear();
    assert_eq!(result(&class),Err(Error::UnsupportedContainer));
}

#[test]
fn hostile_public_model_cp_counts_and_late_method_table_entries_are_revalidated() {
    // A usable forged slot0 must not legalize a consumed zero CP reference.
    for reference in 0..3 {
        let mut class = function();
        match reference {
            0 => { class.constants[0] = utf("pkg/Owner$Nested"); class.constants[15] = Constant::Class(0); }
            1 => { class.constants[0] = utf("call"); class.methods[0].name = 0; }
            _ => { class.constants[0] = utf("()V"); class.methods[0].descriptor = 0; }
        }
        assert_eq!(result(&class), Err(Error::Model(ktlsp_binary::Error::Malformed("constant index zero"))));
    }
    let mut class = function();
    class.constants[18] = Constant::Integer(0);
    assert!(matches!(result(&class),Err(Error::Model(_))));
    let mut class = function(); add_method(&mut class,"other","()V",1);
    class.methods[1].descriptor = 0;
    assert!(matches!(result(&class),Err(Error::Model(_))));
    class.methods[1].descriptor = 18;
    class.methods[1].name = 0;
    assert!(matches!(result(&class),Err(Error::Model(_))));
    let mut class = function(); class.constants[17] = Constant::Utf8(vec![97;65_536]);
    assert!(matches!(result(&class),Err(Error::Model(ktlsp_binary::Error::Limit(_)))));
    let mut class = function(); class.constants[1] = Constant::Utf8(vec![97;65_536]);
    assert!(matches!(result(&class),Err(Error::Model(ktlsp_binary::Error::Limit(_)))));
    let mut class = function(); class.constants.resize_with(65_536,||Constant::Unusable);
    assert!(matches!(result(&class),Err(Error::Model(_))));
    let mut class = function();
    class.methods.resize_with(65_536,||Member {access_flags:1,name:17,descriptor:18,attributes:vec![]});
    assert!(matches!(result(&class),Err(Error::Model(ktlsp_binary::Error::Limit(_)))));
}

fn minimum(class: &ClassFile<'_>, dimension: fn(&mut Limits) -> &mut usize, high: usize) -> usize {
    let mut low = 0; let mut high = high;
    while low < high {
        let middle = low + (high-low)/2;
        let mut limits = Limits::default(); *dimension(&mut limits) = middle;
        if linked_class_method::decode(class,1,limits).is_ok() { high = middle; } else { low = middle+1; }
    }
    let mut limits = Limits::default(); *dimension(&mut limits) = low;
    assert!(linked_class_method::decode(class,1,limits).is_ok());
    assert!(low > 0); *dimension(&mut limits) = low-1;
    assert!(linked_class_method::decode(class,1,limits).is_err());
    low
}
#[test]
fn exact_composite_boundaries_cover_provenance_and_late_table_scans() {
    let class = function();
    let d1_len = result(&class).unwrap().decoded_d1().len();
    let d2_bytes: usize = [12,13,14].iter().map(|index| match &class.constants[*index] {Constant::Utf8(text) => 2*text.len(), _ => unreachable!()}).sum();
    assert_eq!(minimum(&class,|l| &mut l.bytes,16*1024*1024),ATTRIBUTE.len()+d1_len+d2_bytes);
    let work = minimum(&class,|l| &mut l.work,64*1024*1024);
    minimum(&class,|l| &mut l.owned_bytes,16*1024*1024);
    minimum(&class,|l| &mut l.records,65_535);
    minimum(&class,|l| &mut l.nodes,65_535);
    let mut larger = function();
    add_method(&mut larger,"other","()V",1);
    assert!(minimum(&larger,|l| &mut l.work,64*1024*1024) > work);
    let mut limits = Limits::default(); limits.work = work;
    assert!(linked_class_method::decode(&larger,1,limits).is_err());
    let mut limits = Limits::default(); limits.depth = 1;
    assert!(linked_class_method::decode(&class,1,limits).is_err());
}

#[test]
fn annotation_cp_grammar_passes_debit_the_same_work_allowance() {
    let mut class = function();
    class.methods.clear();
    class.constants.push(utf(&format!("L{};", "a".repeat(4094))));
    class.attributes[0].bytes = &[0,1,0,19,0,0];
    let limits = Limits { work: 8192, ..Limits::default() };
    // The six attribute bytes cannot pay for repeated scans of 4096 CP units.
    assert_eq!(linked_class_method::decode(&class,1,limits),
        Err(Error::Model(ktlsp_binary::Error::Limit("work"))));
    assert_eq!(result(&class), Err(Error::UnsupportedContainer));
}
