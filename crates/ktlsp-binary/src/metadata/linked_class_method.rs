//! Finite same-model Class metadata -> original JVM method identity.
use super::{
    Outcome, class_context, class_header, class_id, constructor, function_header, header,
    jvm_method_signature, name_resolver, payload, string_table, wire,
};
use crate::{ClassFile, Constant, Member, annotations, descriptors::{MethodDescriptor, SlotContext}};
use std::ops::Range;

pub use super::package_member_names::Limits;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Constructor,
    Function,
}

/// Encoded presence is retained; a default name is not an explicit extension index.
#[derive(Debug, PartialEq, Eq)]
pub struct MetadataMember {
    pub kind: Kind,
    /// Ordinal among ALL original Class wire fields.
    pub ordinal: usize,
    pub span: Range<usize>,
    pub body_span: Range<usize>,
    pub flags: Option<i32>,
    pub old_flags: Option<i32>,
    pub function_name: Option<i32>,
    pub signature: jvm_method_signature::JvmMethodSignature,
    /// Extension100 body spans, decoded-d1-relative, in original encounter order.
    pub extension_bodies: Vec<Range<usize>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct LinkedMethod<'c, 'data> {
    /// Original supplied model member, never a copied or synthetic declaration.
    pub method: &'c Member<'data>,
    pub method_ordinal: usize,
    pub name: &'c [u16],
    pub descriptor: MethodDescriptor<'c>,
    /// Original this_class -> CONSTANT_Class -> CONSTANT_Utf8, distinct from ClassId.
    pub jvm_owner: &'c [u16],
    pub class_id: class_id::ClassId,
    pub class_span: Range<usize>,
    pub class_flags: Option<i32>,
    pub class_fq_name: i32,
    pub metadata_present: u8,
    pub member: MetadataMember,
    bytes: Vec<u8>,
}
impl LinkedMethod<'_, '_> {
    /// The single owned decoded-d1 buffer; all metadata spans index this allocation.
    pub fn decoded_d1(&self) -> &[u8] {
        &self.bytes
    }
    pub fn member_body(&self) -> Option<&[u8]> {
        self.bytes.get(self.member.body_span.clone())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Model(crate::Error),
    Metadata(super::Error),
    Wire(wire::Error),
    Context(class_context::Error),
    Class(class_header::Error),
    Function(function_header::Error),
    Name(name_resolver::Error),
    UnsupportedContainer,
    MissingOrdinal,
    NotMethod,
    MissingExplicitDescriptor,
    OwnerMismatch,
    UnsupportedOwner,
    SpecialMethod,
    NoMethod,
    DuplicateMethod,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<wire::Error> for Error {
    fn from(error: wire::Error) -> Self { Self::Wire(error) }
}

fn text<'c>(class: &'c ClassFile<'_>, index: u16, budget: &mut string_table::Budget) -> Result<&'c [u16], Error> {
    budget.wire.work(1, 0)?;
    let text = class.utf16(index).map_err(Error::Model)?;
    if text.len() > crate::descriptors::MAX_UNITS {
        return Err(Error::Model(crate::Error::Limit("constant UTF16 units")));
    }
    // CP text validation and descriptor grammar each take at most two linear scans.
    budget.wire.work(text.len() * 3 + 1, 0)?;
    Ok(text)
}

fn owner_matches(owner: &class_id::ClassId, actual: &[u16], budget: &mut string_table::Budget) -> Result<(), Error> {
    let package = owner.package_name();
    let relative = owner.relative_class_name();
    budget.wire.work(4 * (package.len() + relative.len()) + actual.len() + 1, 0)?;
    if owner.local || relative.is_empty() {
        return Err(Error::UnsupportedOwner);
    }
    // ClassId package separators and relative nested separators are different.
    // No reverse '$' guessing, flattened-FQ-name replacement or quoted/local mapping.
    for name in [package, relative] {
        if name.is_empty() { continue; }
        for part in name.split(|unit| *unit == u16::from(b'.')) {
            crate::name_valid(part, false).map_err(|_| Error::UnsupportedOwner)?;
            if part.iter().any(|unit| [60, 62, 96].contains(unit)) {
                return Err(Error::UnsupportedOwner);
            }
        }
    }
    let mapped = package.iter().map(|unit| if *unit == 46 { 47 } else { *unit })
        .chain((!package.is_empty()).then_some(47))
        .chain(relative.iter().map(|unit| if *unit == 46 { 36 } else { *unit }));
    if !mapped.eq(actual.iter().copied()) {
        return Err(Error::OwnerMismatch);
    }
    Ok(())
}

/// Internally extract THIS supplied model's original Metadata and select one Class
/// field8/9 by all-field ordinal, after complete Class framing. The authority is
/// the public mutable ClassFile MODEL, not proof of retained bytes/archive origin.
/// Consumed public model sizes, CP tags/indices, annotations, names and descriptors
/// are revalidated. No foreign Header/Outcome can authorize a method here.
///
/// Only explicit JVM descriptors are supported. Absent extension names follow
/// pinned Kotlin defaults (Function.name or literal <init>); present names win.
/// ALL original methods participate in full UTF16 name+descriptor uniqueness,
/// including return types, bridges and overloads. Actual flags select receiver
/// slots; metadata formal counts are NOT equated to JVM descriptor arity.
///
/// One monotonic budget covers annotation/header/d1/d2, schema/wire/resolver,
/// owner/method scans and all requested capacities. `bytes` includes visited
/// attribute payloads, decoded d1 and d2 UTF16 bytes; output is not an RSS bound.
/// Unselected nested members stay opaque. No source overload/linkage engine,
/// generic erasure, descriptor inference or JVM verification is provided.
pub fn decode<'c, 'data>(
    class: &'c ClassFile<'data>,
    ordinal: usize,
    limits: Limits,
) -> Result<LinkedMethod<'c, 'data>, Error> {
    let mut budget = string_table::Budget::new(string_table::Limits {
        bytes: limits.bytes, records: limits.records, nodes: limits.nodes,
        depth: limits.depth, work: limits.work, owned_bytes: limits.owned_bytes,
    });
    decode_with_budget(class, ordinal, limits, &mut budget)
}

fn decode_with_budget<'c, 'data>(
    class: &'c ClassFile<'data>, ordinal: usize, limits: Limits,
    budget: &mut string_table::Budget,
) -> Result<LinkedMethod<'c, 'data>, Error> {
    let annotations = annotations::decode_with_budget(class, &mut annotations::Budget::composed(budget))
        .map_err(Error::Model)?;
    let metadata = header::decode_with_budget(&annotations, &mut header::Budget::composed(budget))
        .map_err(Error::Metadata)?;
    let Outcome::Decoded { header, bytes } = metadata else { return Err(Error::UnsupportedContainer); };
    if header.k != 1 { return Err(Error::UnsupportedContainer); }
    if header.d2.len() > limits.entries.min(crate::MAX_RECORDS) {
        return Err(Error::Model(crate::Error::Limit("entries")));
    }
    for value in &header.d2 {
        budget.wire.work(1, 0)?;
        budget.input(value.len().checked_mul(2).ok_or(Error::Model(crate::Error::Limit("bytes")))?)?;
    }
    let framed = payload::decode_with_budget(&bytes, budget)?;
    let mut resolver = class_context::resolver(&framed, &header.d2, limits, budget).map_err(Error::Context)?;
    let class_span = framed.payload_span.clone();
    budget.node(1, class_span.start)?;
    let class_header = class_header::decode_with_budget(framed.payload, &mut budget.wire)
        .map_err(|mut error| { error.offset += class_span.start; Error::Class(error) })?;
    let class_id = class_id::resolve_owner(class_header.fq_name, class_span.start, &mut resolver, budget)
        .map_err(|error| match error { class_id::Error::Wire(error) => Error::Wire(error), class_id::Error::Name(error) => Error::Name(error) })?;
    budget.wire.work(1, 0)?;
    if class.this_class == 0 { return Err(Error::Model(crate::Error::Malformed("this_class"))); }
    let Constant::Class(owner_index) = crate::constant(&class.constants, class.this_class).map_err(Error::Model)? else {
        return Err(Error::Model(crate::Error::Malformed("this_class tag")));
    };
    let jvm_owner = text(class, *owner_index, budget)?;
    crate::internal_name(jvm_owner).map_err(Error::Model)?;
    owner_matches(&class_id, jvm_owner, budget)?;
    let mut pos = class_span.start;
    let mut current = 0;
    let mut selected = None;
    while pos < bytes.len() {
        let field = wire::field(&bytes, &mut pos, bytes.len(), &mut budget.wire)?;
        budget.wire.work(1, field.span.start)?;
        if current == ordinal { selected = Some(field); }
        current += 1;
    }
    let field = selected.ok_or(Error::MissingOrdinal)?;
    let (kind, body) = match (field.number, field.value) {
        (8, wire::Value::Bytes(body)) => (Kind::Constructor, body),
        (9, wire::Value::Bytes(body)) => (Kind::Function, body),
        _ => return Err(Error::NotMethod),
    };
    let start = field.span.end - body.len();
    budget.node(2, start)?;
    let (flags, old_flags, function_name) = match kind {
        Kind::Constructor => {
            let parent = constructor::decode_with_budget(body, budget).map_err(|mut error| { error.offset += start; Error::Wire(error) })?;
            (parent.flags, None, None)
        }
        Kind::Function => {
            let parent = function_header::decode_with_budget(body, &mut budget.wire)
                .map_err(|mut error| { error.offset += start; Error::Function(error) })?;
            (parent.flags, parent.old_flags, Some(parent.name))
        }
    };
    let (signature, extension_bodies) = jvm_method_signature::scan_with_budget(body, budget, |_, span| start + span.start..start + span.end)
        .map_err(|mut error| { error.offset += start; Error::Wire(error) })?;
    let signature = signature.ok_or(Error::MissingExplicitDescriptor)?;
    let descriptor_index = signature.desc.ok_or(Error::MissingExplicitDescriptor)?;
    let explicit_name = signature.name.or(function_name);
    let resolved_name = explicit_name.map(|index| resolver.get_string_with_budget(index, budget)).transpose().map_err(Error::Name)?;
    const INIT: &[u16] = &[60, 105, 110, 105, 116, 62];
    let name = resolved_name.as_deref().unwrap_or(INIT);
    let descriptor = resolver.get_string_with_budget(descriptor_index, budget).map_err(Error::Name)?;
    budget.wire.work(name.len() + 2, start)?;
    if match kind { Kind::Constructor => name != INIT, Kind::Function => crate::eq(name, "<init>") || crate::eq(name, "<clinit>") } {
        return Err(Error::SpecialMethod);
    }
    let mut matched = None;
    for (method_ordinal, method) in class.methods.iter().enumerate() {
        budget.wire.work(1, 0)?;
        let actual_name = text(class, method.name, budget)?;
        crate::name_valid(actual_name, true).map_err(Error::Model)?;
        let actual_descriptor = text(class, method.descriptor, budget)?;
        let is_static = method.access_flags & 8 != 0;
        let context = if is_static || crate::eq(actual_name, "<clinit>") { SlotContext::WithoutReceiver } else { SlotContext::WithReceiver };
        let parsed = MethodDescriptor::parse(actual_descriptor, context).map_err(Error::Model)?;
        budget.wire.work(actual_name.len().min(name.len()) + actual_descriptor.len().min(descriptor.len()) + 2, 0)?;
        if actual_name == name && actual_descriptor == descriptor {
            if kind == Kind::Constructor && (is_static || parsed.result().is_some()) { return Err(Error::SpecialMethod); }
            if matched.is_some() { return Err(Error::DuplicateMethod); }
            matched = Some((method_ordinal, method, actual_name, parsed));
        }
    }
    let (method_ordinal, method, name, descriptor) = matched.ok_or(Error::NoMethod)?;
    let class_flags = class_header.flags;
    let class_fq_name = class_header.fq_name;
    let metadata_present = header.present;
    let member = MetadataMember { kind, ordinal, span: field.span, body_span: start..start + body.len(), flags, old_flags, function_name, signature, extension_bodies };
    Ok(LinkedMethod { method, method_ordinal, name, descriptor, jvm_owner, class_id, class_span, class_flags, class_fq_name, metadata_present, member, bytes })
}
