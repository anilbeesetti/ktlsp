//! Explicit decoded-byte mutations of the retained original Kotlin carriers.
//! These are adversarial controls, not additional genuine compiler witnesses.
use ktlsp_binary::metadata::{
    self, class_context, name_resolver, resolved_class_function_parameters as classifier,
    resolved_class_supertypes::{Error, ErrorKind, Limits, decode},
    type_argument::Projection,
    wire,
};
use std::ops::Range;

#[derive(Clone, Copy)]
struct Carrier {
    raw: &'static [u8],
    parameter_start: usize,
    supertype_end: usize,
}
const CARRIERS: [Carrier; 2] = [
    Carrier {
        raw: include_bytes!("fixtures/metadata_class_bounds/ClosedRange.class.bin"),
        parameter_start: 31,
        supertype_end: 51,
    },
    Carrier {
        raw: include_bytes!("fixtures/metadata_class_bounds/ClosedFloatingPointRange.class.bin"),
        parameter_start: 29,
        supertype_end: 55,
    },
];

fn with_input(carrier: Carrier, f: impl FnOnce(metadata::Outcome<'_>)) {
    let class = ktlsp_binary::parse_class(carrier.raw).unwrap();
    let annotations = ktlsp_binary::annotations::decode(&class).unwrap();
    let input = metadata::decode(&annotations, Default::default()).unwrap();
    f(input);
}
fn bytes<'a>(input: &'a metadata::Outcome<'_>) -> &'a [u8] {
    let metadata::Outcome::Decoded { bytes, .. } = input else {
        panic!("original Class carrier must decode")
    };
    bytes
}
fn bytes_mut<'a>(input: &'a mut metadata::Outcome<'_>) -> &'a mut Vec<u8> {
    let metadata::Outcome::Decoded { bytes, .. } = input else {
        panic!("original Class carrier must decode")
    };
    bytes
}
fn borrowed(raw: &[u8], input: &[u8], span: Range<usize>) {
    assert_eq!(raw, &input[span.clone()]);
    assert_eq!(raw.as_ptr(), input[span].as_ptr());
}
fn varint(mut value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    while value >= 128 {
        out.push(value as u8 | 128);
        value >>= 7;
    }
    out.push(value as u8);
    out
}
fn scalar(number: u8, value: i32) -> Vec<u8> {
    let mut out = vec![number * 8];
    out.extend(varint(value as i64 as u64));
    out
}
fn message(number: u8, body: &[u8]) -> Vec<u8> {
    let mut out = vec![number * 8 + 2];
    out.extend(varint(body.len() as u64));
    out.extend(body);
    out
}
fn parameter(id: i32, name: i32, bounds: &[Vec<u8>]) -> Vec<u8> {
    let mut out = scalar(1, id);
    out.extend(scalar(2, name));
    for bound in bounds {
        out.extend(message(5, bound));
    }
    out
}
fn generic(index: i32, argument: &[u8]) -> Vec<u8> {
    let mut out = message(2, argument);
    out.extend(scalar(6, index));
    out
}
fn name_type(index: i32, name: i32) -> Vec<u8> {
    generic(index, &message(2, &scalar(9, name)))
}
fn id_type(index: i32, id: i32) -> Vec<u8> {
    generic(index, &message(2, &scalar(7, id)))
}
fn rewrite(
    input: &mut metadata::Outcome<'_>,
    carrier: Carrier,
    parameters: &[Vec<u8>],
    supertypes: &[Vec<u8>],
) {
    // One replacement per freshly parsed original; retain table, owner and all
    // unrelated original Class fields. Never manufacture an Outcome directly.
    let mut replacement = Vec::new();
    for p in parameters {
        replacement.extend(message(5, p));
    }
    for ty in supertypes {
        replacement.extend(message(6, ty));
    }
    bytes_mut(input).splice(carrier.parameter_start..carrier.supertype_end, replacement);
}
fn failure(input: &metadata::Outcome<'_>) -> Error {
    decode(input, Limits::default()).unwrap_err()
}
fn unsupported_type(input: &metadata::Outcome<'_>) {
    assert!(matches!(
        failure(input).kind,
        ErrorKind::UnsupportedType | ErrorKind::Classifier(classifier::ErrorKind::UnsupportedType)
    ));
}

#[test]
fn immediate_class_name_lookup_is_required_unique_and_lossless_utf16() {
    const LOSSLESS: &[u16] = &[0xd800, 0, 0xdc00, 84];
    const DIFFERENT: &[u16] = &[0xd800, 0, 0xdc01, 84];
    for carrier in CARRIERS {
        for bound in [false, true] {
            with_input(carrier, |mut input| {
                let p = parameter(91, 1, if bound { &[name_type(2, 0)] } else { &[] });
                rewrite(&mut input, carrier, &[p], &[name_type(3, 0)]);
                assert_eq!(
                    failure(&input).kind,
                    ErrorKind::Classifier(classifier::ErrorKind::UnknownParameterName)
                );
            });
            with_input(carrier, |mut input| {
                let first = parameter(91, 1, if bound { &[name_type(2, 1)] } else { &[] });
                let second = parameter(-7, 1, &[]);
                rewrite(&mut input, carrier, &[first, second], &[name_type(3, 1)]);
                assert_eq!(
                    failure(&input).kind,
                    ErrorKind::Classifier(classifier::ErrorKind::AmbiguousParameterName)
                );
            });
        }
        with_input(carrier, |mut input| {
            // Indices 1 and 4 use d2 fallback in BOTH original compact tables.
            // Different indices alias equal UTF16, including NUL/unpaired surrogates.
            let metadata::Outcome::Decoded { header, .. } = &mut input else {
                unreachable!()
            };
            header.d2[1] = LOSSLESS;
            header.d2[4] = LOSSLESS;
            rewrite(
                &mut input,
                carrier,
                &[parameter(91, 1, &[name_type(2, 4)])],
                &[name_type(3, 4)],
            );
            let out = decode(&input, Limits::default()).unwrap();
            assert_eq!(out.parameters[0].name, LOSSLESS);
            for ty in [&out.bounds[0].ty, &out.supertypes[0]] {
                assert_eq!(ty.arguments[0].header.type_parameter_name, Some(4));
                assert_eq!(ty.arguments[0].classifier, classifier::Classifier::ClassParameter(0));
                assert_eq!(ty.arguments[0].header.type_parameter, None);
            }
            drop(out);
            let metadata::Outcome::Decoded { header, .. } = &mut input else {
                unreachable!()
            };
            header.d2[4] = DIFFERENT;
            assert_eq!(
                failure(&input).kind,
                ErrorKind::Classifier(classifier::ErrorKind::UnknownParameterName)
            );
        });
        with_input(carrier, |mut input| {
            // A reference without ANY Class declaration cannot become Function-own.
            rewrite(&mut input, carrier, &[], &[name_type(3, 1)]);
            assert_eq!(
                failure(&input).kind,
                ErrorKind::Classifier(classifier::ErrorKind::UnknownParameterName)
            );
        });
    }
}

#[test]
fn signed_ids_are_original_ids_not_positions_or_name_scope() {
    for carrier in CARRIERS {
        with_input(carrier, |mut input| {
            rewrite(
                &mut input,
                carrier,
                &[
                    parameter(91, 1, &[id_type(2, -7)]),
                    parameter(-7, 4, &[id_type(2, 91)]),
                ],
                &[id_type(3, -7), id_type(3, 91)],
            );
            let out = decode(&input, Limits::default()).unwrap();
            assert_eq!(out.parameters[0].parameter.id, 91);
            assert_eq!(out.parameters[1].parameter.id, -7);
            assert_eq!(out.bounds.iter().map(|b| b.parameter).collect::<Vec<_>>(), [0, 1]);
            for (ty, ordinal, id) in [
                (&out.bounds[0].ty, 1, -7),
                (&out.bounds[1].ty, 0, 91),
                (&out.supertypes[0], 1, -7),
                (&out.supertypes[1], 0, 91),
            ] {
                let a = &ty.arguments[0];
                assert_eq!(a.classifier, classifier::Classifier::ClassParameter(ordinal));
                assert_eq!(a.header.type_parameter, Some(id));
                assert_eq!(a.header.type_parameter_name, None);
                borrowed(a.header.raw, bytes(&input), a.type_span.clone());
            }
        });
        for id in [0, 1, -1, i32::MIN] {
            for bound in [false, true] {
                with_input(carrier, |mut input| {
                    let bounds = if bound { vec![id_type(2, id)] } else { vec![] };
                    rewrite(
                        &mut input,
                        carrier,
                        &[parameter(91, 1, &bounds), parameter(-7, 4, &[])],
                        &[id_type(3, id)],
                    );
                    assert_eq!(
                        failure(&input).kind,
                        ErrorKind::Classifier(classifier::ErrorKind::UnknownParameterId)
                    );
                });
            }
        }
    }
}

#[test]
fn invalid_classifier_indices_and_conflicting_or_nominal_leaves_refuse() {
    for carrier in CARRIERS {
        for (root, leaf) in [(127, scalar(9, 1)), (2, scalar(9, 127))] {
            for bound in [false, true] {
                with_input(carrier, |mut input| {
                    let ty = generic(root, &message(2, &leaf));
                    let bounds = if bound { vec![ty.clone()] } else { vec![] };
                    rewrite(&mut input, carrier, &[parameter(0, 1, &bounds)], &[ty]);
                    assert_eq!(
                        failure(&input).kind,
                        ErrorKind::Classifier(classifier::ErrorKind::Name(name_resolver::Error {
                            kind: name_resolver::ErrorKind::InvalidIndex,
                            index: Some(127),
                            record: None,
                        }))
                    );
                });
            }
        }
        for leaf in [
            vec![],
            scalar(6, 2),
            [scalar(7, 0), scalar(9, 1)].concat(),
            [scalar(6, 2), scalar(7, 0)].concat(),
            [scalar(9, 1), scalar(1, 1)].concat(),
            [scalar(9, 1), message(2, &[])].concat(),
        ] {
            for bound in [false, true] {
                with_input(carrier, |mut input| {
                    let ty = generic(2, &message(2, &leaf));
                    let bounds = if bound { vec![ty.clone()] } else { vec![] };
                    rewrite(&mut input, carrier, &[parameter(0, 1, &bounds)], &[ty]);
                    unsupported_type(&input);
                });
            }
        }
    }
}

#[test]
fn every_significant_wrong_wire_and_richer_type_shape_refuses() {
    for carrier in CARRIERS {
        // Allowed root scalar/argument numbers must not become allowed wrong wires.
        let mut roots = vec![vec![], scalar(7, 0), scalar(9, 1)];
        for field in [1, 3, 6] {
            roots.push([scalar(6, 2), message(field, &[])].concat());
        }
        roots.push([scalar(6, 2), scalar(2, 0)].concat());
        roots.push([scalar(6, 2), scalar(1, 1)].concat());
        for field in [4, 5, 7, 8, 9, 10, 11, 12, 13, 14] {
            roots.push([scalar(6, 2), scalar(field, 0)].concat());
            roots.push([scalar(6, 2), message(field, &[])].concat());
        }
        for ty in roots {
            for bound in [false, true] {
                with_input(carrier, |mut input| {
                    let bounds = if bound { vec![ty.clone()] } else { vec![] };
                    rewrite(&mut input, carrier, &[parameter(0, 1, &bounds)], &[ty.clone()]);
                    unsupported_type(&input);
                });
            }
        }
        for field in [1, 3, 6, 7, 9] {
            with_input(carrier, |mut input| {
                let leaf = [scalar(9, 1), message(field, &[])].concat();
                rewrite(
                    &mut input,
                    carrier,
                    &[parameter(0, 1, &[generic(2, &message(2, &leaf))])],
                    &[],
                );
                unsupported_type(&input);
            });
        }
        for field in [2, 4, 5, 8, 10, 11, 12, 13, 14] {
            for extra in [scalar(field, 0), message(field, &[])] {
                with_input(carrier, |mut input| {
                    let leaf = [scalar(9, 1), extra].concat();
                    rewrite(
                        &mut input,
                        carrier,
                        &[parameter(0, 1, &[])],
                        &[generic(3, &message(2, &leaf))],
                    );
                    unsupported_type(&input);
                });
            }
        }
        with_input(carrier, |mut input| {
            rewrite(&mut input, carrier, &[parameter(0, 1, &[])], &[]);
            bytes_mut(&mut input).extend(scalar(6, 0));
            assert_eq!(failure(&input).kind, ErrorKind::UnsupportedClass);
        });
        with_input(carrier, |mut input| {
            let p = [parameter(0, 1, &[]), scalar(5, 0)].concat();
            rewrite(&mut input, carrier, &[p], &[]);
            assert_eq!(failure(&input).kind, ErrorKind::UnsupportedParameter);
        });
    }
}

#[test]
fn argument_projection_presence_missing_role_and_duplicate_singular_bodies() {
    for carrier in CARRIERS {
        for bound in [false, true] {
            for projection in [0, 1, 3, 99, -1] {
                with_input(carrier, |mut input| {
                    let argument = [scalar(1, projection), message(2, &scalar(9, 1))].concat();
                    let ty = generic(2, &argument);
                    let bounds = if bound { vec![ty.clone()] } else { vec![] };
                    rewrite(&mut input, carrier, &[parameter(0, 1, &bounds)], &[ty]);
                    assert_eq!(failure(&input).kind, ErrorKind::UnsupportedProjection);
                });
            }
            for argument in [message(1, &[]), vec![0x0d, 0, 0, 0, 0]] {
                with_input(carrier, |mut input| {
                    let ty = generic(2, &argument);
                    let bounds = if bound { vec![ty.clone()] } else { vec![] };
                    rewrite(&mut input, carrier, &[parameter(0, 1, &bounds)], &[ty]);
                    assert_eq!(failure(&input).kind, ErrorKind::UnsupportedProjection);
                });
            }
            for argument in [vec![], scalar(1, 2)] {
                with_input(carrier, |mut input| {
                    let ty = generic(2, &argument);
                    let bounds = if bound { vec![ty.clone()] } else { vec![] };
                    rewrite(&mut input, carrier, &[parameter(0, 1, &bounds)], &[ty]);
                    assert_eq!(failure(&input).kind, ErrorKind::MissingRole);
                });
            }
            for second in [scalar(9, 1), vec![], vec![0x80]] {
                with_input(carrier, |mut input| {
                    let argument = [message(2, &scalar(9, 1)), message(2, &second)].concat();
                    let ty = generic(2, &argument);
                    let bounds = if bound { vec![ty.clone()] } else { vec![] };
                    rewrite(&mut input, carrier, &[parameter(0, 1, &bounds)], &[ty]);
                    assert_eq!(failure(&input).kind, ErrorKind::DuplicateInlineRole);
                });
            }
            with_input(carrier, |mut input| {
                let ty = generic(2, &scalar(2, 0));
                let bounds = if bound { vec![ty.clone()] } else { vec![] };
                rewrite(&mut input, carrier, &[parameter(0, 1, &bounds)], &[ty]);
                unsupported_type(&input);
            });
        }
        with_input(carrier, |mut input| {
            let mut argument = scalar(1, 2);
            argument.extend(message(2, &scalar(9, 1)));
            // Unknown extensions remain bounded opaque data, not richer Type roles.
            argument.extend(message(15, &[0xff]));
            let mut ty = generic(2, &argument);
            ty.extend(scalar(1, 0));
            ty.extend(scalar(3, 1));
            ty.extend(message(15, &[0xff]));
            rewrite(&mut input, carrier, &[parameter(0, 1, &[ty.clone()])], &[ty]);
            let out = decode(&input, Limits::default()).unwrap();
            for ty in [&out.bounds[0].ty, &out.supertypes[0]] {
                assert_eq!(ty.header.flags, Some(0));
                assert_eq!(ty.header.nullable, Some(true));
                let a = &ty.arguments[0];
                assert_eq!(a.raw.projection, Some(Projection::Inv));
                assert_eq!(a.raw.type_id, None);
                assert_eq!(a.raw.inline_types.len(), 1);
                borrowed(a.raw.raw, bytes(&input), a.body_span.clone());
                borrowed(a.header.raw, bytes(&input), a.type_span.clone());
                borrowed(a.raw.inline_types[0].raw, a.raw.raw, a.raw.inline_types[0].span.clone());
            }
        });
    }
}

#[test]
fn bounds_are_parameter_major_and_all_original_repetitions_keep_order_and_custody() {
    for carrier in CARRIERS {
        with_input(carrier, |mut input| {
            let first = id_type(2, 91);
            let second = id_type(3, 91);
            let mut p = parameter(91, 1, &[first.clone()]);
            p.extend(scalar(15, 9)); // Counts toward the ALL-TypeParameter-field ordinal.
            p.extend(message(5, &second));
            p.extend(message(5, &first)); // Equal repeated bound is not deduplicated.
            rewrite(
                &mut input,
                carrier,
                &[p],
                &[id_type(3, 91), id_type(2, -7), id_type(3, 91)],
            );
            // Place a later declaration AFTER all selected supertypes and original
            // unrelated fields. Complete Class scope must be known before decoding.
            let later_start = bytes(&input).len();
            bytes_mut(&mut input).extend(message(5, &parameter(-7, 4, &[id_type(2, -7)])));
            let out = decode(&input, Limits::default()).unwrap();
            assert_eq!(out.parameters.len(), 2);
            assert_eq!(out.parameters[1].body_span.start, later_start + 2);
            assert_eq!(out.bounds.len(), 4);
            for (b, parameter, ordinal, index) in [
                (&out.bounds[0], 0, 2, 2),
                (&out.bounds[1], 0, 4, 3),
                (&out.bounds[2], 0, 5, 2),
                (&out.bounds[3], 1, 2, 2),
            ] {
                assert_eq!(b.parameter, parameter);
                assert_eq!(b.ty.ordinal, ordinal);
                assert_eq!(b.ty.header.class_name, Some(index));
                borrowed(b.ty.header.raw, bytes(&input), b.ty.body_span.clone());
                let original = &out.parameters[parameter];
                let selected = &original.parameter.upper_bound[match ordinal {
                    4 => 1,
                    5 => 2,
                    _ => 0,
                }];
                assert_eq!(b.ty.body_span.start, original.body_span.start + selected.span.start);
                assert_eq!(b.ty.header.raw.as_ptr(), selected.raw.as_ptr());
                assert_eq!(b.ty.span.start + 2, b.ty.body_span.start);
                assert_eq!(b.ty.span.end, b.ty.body_span.end);
            }
            for (s, ordinal, index, parameter) in [
                (&out.supertypes[0], 3, 3, 0),
                (&out.supertypes[1], 4, 2, 1),
                (&out.supertypes[2], 5, 3, 0),
            ] {
                assert_eq!(s.ordinal, ordinal);
                assert_eq!(s.header.class_name, Some(index));
                assert_eq!(s.arguments[0].classifier, classifier::Classifier::ClassParameter(parameter));
                borrowed(s.header.raw, bytes(&input), s.body_span.clone());
            }
            assert_eq!(out.supertypes[0].class_id, out.supertypes[2].class_id);
            assert_ne!(out.supertypes[0].class_id, out.supertypes[1].class_id);
        });
        with_input(carrier, |mut input| {
            rewrite(&mut input, carrier, &[parameter(0, 1, &[])], &[]);
            let out = decode(&input, Limits::default()).unwrap();
            assert!(out.bounds.is_empty());
            assert!(out.supertypes.is_empty()); // No implicit Any/Any? or inferred bound.
            assert!(out.parameters[0].parameter.upper_bound.is_empty());
        });
    }
}

#[test]
fn selected_table_roles_refuse_id_only_mixed_empty_malformed_and_wrong_wire_forms() {
    for carrier in CARRIERS {
        for mixed in [false, true] {
            for role in [scalar(2, 0), message(2, &[]), message(2, &[0x80]), vec![0x15, 0, 0, 0, 0]] {
                with_input(carrier, |mut input| {
                    let supers = if mixed { vec![name_type(3, 1)] } else { vec![] };
                    rewrite(&mut input, carrier, &[parameter(0, 1, &[name_type(2, 1)])], &supers);
                    bytes_mut(&mut input).extend(role);
                    assert_eq!(failure(&input).kind, ErrorKind::UnsupportedTableRole);
                });
            }
            for role in [scalar(6, 0), message(6, &[]), message(6, &[0x80]), vec![0x35, 0, 0, 0, 0]] {
                with_input(carrier, |mut input| {
                    let bounds = if mixed { vec![name_type(2, 1)] } else { vec![] };
                    let mut p = parameter(0, 1, &bounds);
                    p.extend(&role);
                    rewrite(&mut input, carrier, &[p], &[name_type(3, 1)]);
                    let error = failure(&input);
                    if role == message(6, &[0x80]) {
                        // Complete original declarations are decoded before selection;
                        // malformed packed IDs may fail in that existing reader first.
                        assert!(matches!(error.kind,
                            ErrorKind::UnsupportedTableRole
                            | ErrorKind::Context(class_context::ErrorKind::TypeParameter(
                                metadata::type_parameter::ErrorKind::Wire(wire::ErrorKind::Truncated)
                            ))
                        ));
                    } else {
                        assert_eq!(error.kind, ErrorKind::UnsupportedTableRole);
                    }
                });
            }
            for role in [scalar(3, 0), message(3, &[]), message(3, &[0x80]), vec![0x1d, 0, 0, 0, 0]] {
                for bound in [false, true] {
                    with_input(carrier, |mut input| {
                        let mut argument = if mixed { message(2, &scalar(9, 1)) } else { vec![] };
                        argument.extend(&role);
                        let ty = generic(2, &argument);
                        let bounds = if bound { vec![ty.clone()] } else { vec![] };
                        rewrite(&mut input, carrier, &[parameter(0, 1, &bounds)], &[ty]);
                        assert_eq!(failure(&input).kind, ErrorKind::UnsupportedTableRole);
                    });
                }
            }
        }
        with_input(carrier, |mut input| {
            // A genuinely unused table body and unrelated nested declaration remain
            // opaque; selected table ROLE refusal must not reject table presence.
            bytes_mut(&mut input).extend([0xf2, 1, 1, 0x80]); // Class TypeTable field30.
            bytes_mut(&mut input).extend(message(9, &[0x80])); // Unselected Function.
            assert!(decode(&input, Limits::default()).is_ok());
        });
    }
}

#[test]
fn malformed_late_class_and_selected_children_never_return_successful_prefixes() {
    for carrier in CARRIERS {
        with_input(carrier, |mut input| {
            let end = bytes(&input).len();
            bytes_mut(&mut input).push(0x80);
            let error = failure(&input);
            assert_eq!(error.offset, end + 1);
            assert_eq!(error.kind, ErrorKind::Context(class_context::ErrorKind::Class(
                metadata::class_header::ErrorKind::Wire(wire::ErrorKind::Truncated)
            )));
        });
        with_input(carrier, |mut input| {
            let end = bytes(&input).len();
            bytes_mut(&mut input).extend(message(6, &[0x80]));
            let error = failure(&input);
            assert_eq!(error.kind, ErrorKind::Wire(wire::ErrorKind::Truncated));
            assert_eq!(error.offset, end + 3);
        });
        for bad_bound in [vec![0x80], generic(2, &message(2, &[0x80]))] {
            with_input(carrier, |mut input| {
                rewrite(
                    &mut input,
                    carrier,
                    &[parameter(0, 1, &[name_type(2, 1), bad_bound])],
                    &[name_type(3, 1)],
                );
                assert_eq!(failure(&input).kind, ErrorKind::Wire(wire::ErrorKind::Truncated));
            });
        }
        with_input(carrier, |mut input| {
            bytes_mut(&mut input).extend(message(6, &generic(3, &[0x80])));
            assert_eq!(failure(&input).kind, ErrorKind::Wire(wire::ErrorKind::Truncated));
        });
    }
}

type Set = fn(&mut Limits, usize);
const DIMENSIONS: [(&str, Set); 10] = [
    ("bytes", |l, n| l.bytes = n),
    ("records", |l, n| l.records = n),
    ("nodes", |l, n| l.nodes = n),
    ("depth", |l, n| l.depth = n),
    ("work", |l, n| l.work = n),
    ("owned_bytes", |l, n| l.owned_bytes = n),
    ("queries", |l, n| l.queries = n),
    ("output_units", |l, n| l.output_units = n),
    ("entries", |l, n| l.entries = n),
    ("range_slots", |l, n| l.range_slots = n),
];
fn minimum(input: &metadata::Outcome<'_>, set: Set) -> usize {
    // Measured only when THIS new composed operation is eventually admitted/run.
    // No prior operation's minima or fixed accounting totals are reused.
    let accepted = |n| {
        let mut limits = Limits::default();
        set(&mut limits, n);
        decode(input, limits).is_ok()
    };
    let (mut low, mut high) = (0, 1_000_000);
    assert!(!accepted(low));
    assert!(accepted(high));
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if accepted(middle) {
            high = middle;
        } else {
            low = middle;
        }
    }
    high
}
fn assert_limit(error: Error, label: &'static str) {
    let found = match error.kind {
        ErrorKind::Wire(wire::ErrorKind::Limit(found))
        | ErrorKind::Context(class_context::ErrorKind::Wire(wire::ErrorKind::Limit(found)))
        | ErrorKind::Context(class_context::ErrorKind::Class(
            metadata::class_header::ErrorKind::Wire(wire::ErrorKind::Limit(found)),
        ))
        | ErrorKind::Context(class_context::ErrorKind::TypeParameter(
            metadata::type_parameter::ErrorKind::Wire(wire::ErrorKind::Limit(found)),
        ))
        | ErrorKind::Classifier(classifier::ErrorKind::Wire(wire::ErrorKind::Limit(found))) => found,
        ErrorKind::Classifier(classifier::ErrorKind::Name(name_resolver::Error {
            kind: name_resolver::ErrorKind::Limit(found), ..
        }))
        | ErrorKind::Context(class_context::ErrorKind::Name(name_resolver::Error {
            kind: name_resolver::ErrorKind::Limit(found), ..
        })) => found,
        other => panic!("expected {label} exhaustion, got {other:?} at {}", error.offset),
    };
    assert_eq!(found, label);
}

#[test]
fn new_operation_measures_joint_exact_and_every_joint_one_short_allowance() {
    for carrier in CARRIERS {
        with_input(carrier, |input| {
            let minima: Vec<_> = DIMENSIONS.iter().map(|(_, set)| minimum(&input, *set)).collect();
            let mut exact = Limits::default();
            for ((_, set), n) in DIMENSIONS.iter().zip(&minima) {
                set(&mut exact, *n);
            }
            assert_eq!(decode(&input, exact).unwrap(), decode(&input, Limits::default()).unwrap());
            for ((label, set), n) in DIMENSIONS.iter().zip(&minima) {
                let mut below = exact;
                set(&mut below, n - 1);
                assert_limit(decode(&input, below).unwrap_err(), label);
            }
        });
    }
}

#[test]
fn bounds_and_supertypes_share_one_resolver_and_later_children_never_reset_resources() {
    for carrier in CARRIERS {
        // Charge bounds and supertypes against the SAME resolver, not two public
        // operations each starting a new query/output allowance.
        for (label, set) in [DIMENSIONS[6], DIMENSIONS[7]] {
            let mut separate = [0, 0];
            for (slot, bound_only) in [true, false].into_iter().enumerate() {
                with_input(carrier, |mut input| {
                    let bounds = if bound_only { vec![name_type(2, 1)] } else { vec![] };
                    let supers = if bound_only { vec![] } else { vec![name_type(3, 1)] };
                    rewrite(&mut input, carrier, &[parameter(0, 1, &bounds)], &supers);
                    separate[slot] = minimum(&input, set);
                });
            }
            with_input(carrier, |mut input| {
                rewrite(
                    &mut input,
                    carrier,
                    &[parameter(0, 1, &[name_type(2, 1)])],
                    &[name_type(3, 1)],
                );
                let joint = minimum(&input, set);
                assert!(joint > separate[0].max(separate[1]), "shared {label}");
                let mut limited = Limits::default();
                set(&mut limited, separate[0].max(separate[1]));
                assert_limit(decode(&input, limited).unwrap_err(), label);
            });
        }
        for (label, set) in [DIMENSIONS[1], DIMENSIONS[2], DIMENSIONS[4], DIMENSIONS[5], DIMENSIONS[6], DIMENSIONS[7]] {
            with_input(carrier, |mut input| {
                // A single selected child determines a fresh allowance. A second
                // occurrence must consume MORE, even when its bytes/identity repeat.
                rewrite(&mut input, carrier, &[parameter(0, 1, &[name_type(2, 1)])], &[]);
                let first = minimum(&input, set);
                let later_start = bytes(&input).len();
                bytes_mut(&mut input).extend(message(6, &name_type(3, 1)));
                assert!(minimum(&input, set) > first, "cumulative {label}");
                let mut limited = Limits::default();
                set(&mut limited, first);
                let error = decode(&input, limited).unwrap_err();
                if matches!(label, "queries" | "output_units" | "nodes") {
                    assert!(error.offset >= later_start, "later child {label}: {error:?}");
                }
                assert_limit(error, label);
            });
            with_input(carrier, |mut input| {
                rewrite(&mut input, carrier, &[parameter(0, 1, &[])], &[name_type(3, 1)]);
                let first = minimum(&input, set);
                bytes_mut(&mut input).extend(message(6, &name_type(3, 1)));
                assert!(minimum(&input, set) > first, "repeated supertype {label}");
                let mut limited = Limits::default();
                set(&mut limited, first);
                assert_limit(decode(&input, limited).unwrap_err(), label);
            });
        }
    }
}

#[test]
fn repeated_arguments_keep_order_and_consume_cumulative_child_allowances() {
    let dimensions = [
        DIMENSIONS[1], DIMENSIONS[2], DIMENSIONS[4],
        DIMENSIONS[5], DIMENSIONS[6], DIMENSIONS[7],
    ];
    for carrier in CARRIERS {
        for bound in [false, true] {
            let mut minima = [0; 6];
            for count in [2, 3] {
                with_input(carrier, |mut input| {
                    let mut ty = Vec::new();
                    for index in [1, 4, 1].into_iter().take(count) {
                        ty.extend(message(2, &message(2, &scalar(9, index))));
                    }
                    ty.extend(scalar(6, 2));
                    let bounds = if bound { vec![ty.clone()] } else { vec![] };
                    let supers = if bound { vec![] } else { vec![ty] };
                    rewrite(
                        &mut input,
                        carrier,
                        &[parameter(91, 1, &bounds), parameter(-7, 4, &[])],
                        &supers,
                    );
                    let out = decode(&input, Limits::default()).unwrap();
                    let selected = if bound { &out.bounds[0].ty } else { &out.supertypes[0] };
                    assert_eq!(selected.arguments.len(), count);
                    for (argument, ordinal) in selected.arguments.iter().zip([0, 1, 0]) {
                        assert_eq!(argument.classifier, classifier::Classifier::ClassParameter(ordinal));
                        assert_eq!(argument.raw.projection, None);
                        assert_eq!(argument.raw.type_id, None);
                        borrowed(argument.raw.raw, bytes(&input), argument.body_span.clone());
                        borrowed(argument.header.raw, bytes(&input), argument.type_span.clone());
                    }
                    assert!(selected.arguments.windows(2).all(|pair| {
                        pair[0].body_span.end <= pair[1].body_span.start
                    }));
                    let last_leaf = selected.arguments.last().unwrap().type_span.start;
                    drop(out);
                    for (slot, (label, set)) in dimensions.into_iter().enumerate() {
                        let required = minimum(&input, set);
                        if count == 2 {
                            minima[slot] = required;
                        } else {
                            assert!(required > minima[slot], "later Argument {label}");
                            let mut limited = Limits::default();
                            set(&mut limited, minima[slot]);
                            let error = decode(&input, limited).unwrap_err();
                            if matches!(label, "queries" | "output_units") {
                                assert_eq!(error.offset, last_leaf, "later Argument {label}");
                            }
                            assert_limit(error, label);
                        }
                    }
                });
            }
        }
    }
}

#[test]
fn mixed_table_roles_before_inline_children_cannot_be_overwritten_by_inline_success() {
    for carrier in CARRIERS {
        with_input(carrier, |mut input| {
            let mut p = parameter(0, 1, &[]);
            p.extend(scalar(6, 0));
            p.extend(message(5, &name_type(2, 1)));
            rewrite(&mut input, carrier, &[p], &[name_type(3, 1)]);
            assert_eq!(failure(&input).kind, ErrorKind::UnsupportedTableRole);
        });
        with_input(carrier, |mut input| {
            rewrite(
                &mut input,
                carrier,
                &[parameter(0, 1, &[name_type(2, 1)])],
                &[name_type(3, 1)],
            );
            bytes_mut(&mut input).splice(
                carrier.parameter_start..carrier.parameter_start,
                scalar(2, 0),
            );
            assert_eq!(failure(&input).kind, ErrorKind::UnsupportedTableRole);
        });
        for bound in [false, true] {
            with_input(carrier, |mut input| {
                let argument = [scalar(3, 0), message(2, &scalar(9, 1))].concat();
                let ty = generic(2, &argument);
                let bounds = if bound { vec![ty.clone()] } else { vec![] };
                rewrite(&mut input, carrier, &[parameter(0, 1, &bounds)], &[ty]);
                assert_eq!(failure(&input).kind, ErrorKind::UnsupportedTableRole);
            });
        }
    }
}

#[test]
fn significant_declaration_wires_refuse_and_scalar_presence_is_preserved() {
    for carrier in CARRIERS {
        for field in 1..=4 {
            with_input(carrier, |mut input| {
                let mut p = parameter(0, 1, &[name_type(2, 1)]);
                p.extend(message(field, &[]));
                rewrite(&mut input, carrier, &[p], &[name_type(3, 1)]);
                assert_eq!(
                    failure(&input).kind,
                    if field <= 2 {
                        ErrorKind::Context(class_context::ErrorKind::UnsupportedParameter)
                    } else {
                        ErrorKind::UnsupportedParameter
                    }
                );
            });
        }
        with_input(carrier, |mut input| {
            let mut p = parameter(0, 1, &[name_type(2, 1)]);
            p.extend(scalar(3, 0));
            p.extend(scalar(4, 2));
            p.extend(scalar(4, 99)); // Unknown enum does not erase known explicit INV.
            rewrite(&mut input, carrier, &[p], &[name_type(3, 1)]);
            let out = decode(&input, Limits::default()).unwrap();
            assert_eq!(out.parameters[0].parameter.reified, Some(false));
            assert_eq!(
                out.parameters[0].parameter.variance,
                Some(metadata::type_parameter::Variance::Inv)
            );
            borrowed(
                out.parameters[0].parameter.raw,
                bytes(&input),
                out.parameters[0].body_span.clone(),
            );
            assert_eq!(out.bounds.len(), 1);
            assert_eq!(out.bounds[0].parameter, 0);
        });
    }
}
