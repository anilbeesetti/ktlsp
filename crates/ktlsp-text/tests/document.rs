use ktlsp_text::{
    Document, MAX_CHANGES, MAX_DOCUMENT_BYTES, Position, Range, TextChange, TextError,
    offset_to_position, position_to_offset,
};

fn pos(line: u32, character: u32) -> Position {
    Position { line, character }
}

fn change(start: Position, end: Position, range_length: Option<u32>, text: &str) -> TextChange {
    TextChange {
        range: Some(Range { start, end }),
        range_length,
        text: text.into(),
    }
}

fn full(text: &str) -> TextChange {
    TextChange {
        range: None,
        range_length: None,
        text: text.into(),
    }
}

#[test]
fn unicode_coordinates_count_utf16_and_combining_marks() {
    let text = "a😀e\u{301}水";
    for (units, bytes) in [(0, 0), (1, 1), (3, 5), (4, 6), (5, 8), (6, 11)] {
        assert_eq!(position_to_offset(text, pos(0, units)), Ok(bytes));
        assert_eq!(offset_to_position(text, bytes), Ok(pos(0, units)));
    }
    assert_eq!(
        position_to_offset(text, pos(0, 2)),
        Err(TextError::InvalidPosition(pos(0, 2)))
    );
    assert!(position_to_offset(text, pos(0, 7)).is_err());
    for offset in [2, 3, 4, 7, 9, 10, 12, usize::MAX] {
        assert_eq!(
            offset_to_position(text, offset),
            Err(TextError::InvalidOffset(offset))
        );
    }
}

#[test]
fn all_newline_forms_and_final_empty_line() {
    let text = "a\r\n😀\rb\n";
    for (offset, position) in [
        (0, pos(0, 0)),
        (1, pos(0, 1)),
        (3, pos(1, 0)),
        (7, pos(1, 2)),
        (8, pos(2, 0)),
        (9, pos(2, 1)),
        (10, pos(3, 0)),
    ] {
        assert_eq!(offset_to_position(text, offset), Ok(position));
        assert_eq!(position_to_offset(text, position), Ok(offset));
    }
    assert_eq!(
        offset_to_position(text, 2),
        Err(TextError::InvalidOffset(2))
    );
    for invalid in [
        pos(0, 2),
        pos(1, 1),
        pos(2, 2),
        pos(3, 1),
        pos(4, 0),
        pos(u32::MAX, u32::MAX),
    ] {
        assert_eq!(
            position_to_offset(text, invalid),
            Err(TextError::InvalidPosition(invalid))
        );
    }
}

#[test]
fn empty_and_adjacent_line_terminators() {
    assert_eq!(position_to_offset("", pos(0, 0)), Ok(0));
    assert_eq!(offset_to_position("", 0), Ok(pos(0, 0)));
    assert!(position_to_offset("", pos(1, 0)).is_err());
    let text = "\r\r\n\n";
    for (offset, position) in [
        (0, pos(0, 0)),
        (1, pos(1, 0)),
        (3, pos(2, 0)),
        (4, pos(3, 0)),
    ] {
        assert_eq!(offset_to_position(text, offset), Ok(position));
        assert_eq!(position_to_offset(text, position), Ok(offset));
    }
    assert!(offset_to_position(text, 2).is_err());
}

#[test]
fn sequential_changes_use_the_result_of_previous_changes() {
    let mut document = Document::new(-10, "a😀\r\nb".into()).unwrap();
    document
        .apply_changes(
            -9,
            &[
                change(pos(0, 1), pos(0, 3), Some(2), "x\n"),
                change(pos(1, 0), pos(2, 1), Some(3), "水"),
                change(pos(1, 1), pos(1, 1), Some(0), "!"),
            ],
        )
        .unwrap();
    assert_eq!(document.text(), "ax\n水!");
    assert_eq!(document.version(), -9);
}

#[test]
fn full_changes_can_mix_with_incremental_and_empty_changes() {
    let mut document = Document::new(1, "old".into()).unwrap();
    document
        .apply_changes(
            9,
            &[
                full("😀\r\n"),
                change(pos(1, 0), pos(1, 0), None, "new"),
                full(""),
                change(pos(0, 0), pos(0, 0), Some(0), "done"),
            ],
        )
        .unwrap();
    assert_eq!(document.text(), "done");
    document.apply_changes(10, &[]).unwrap();
    assert_eq!(document.text(), "done");
    assert_eq!(document.version(), 10);
}

#[test]
fn stale_duplicate_and_extreme_versions_never_mutate() {
    let mut document = Document::new(i32::MAX - 1, "text".into()).unwrap();
    for proposed in [i32::MIN, 0, i32::MAX - 1] {
        let before = document.clone();
        assert!(matches!(
            document.apply_changes(proposed, &[full("bad")]),
            Err(TextError::StaleVersion { .. })
        ));
        assert_eq!(document, before);
    }
    document.apply_changes(i32::MAX, &[full("last")]).unwrap();
    let before = document.clone();
    assert!(document.apply_changes(i32::MAX, &[]).is_err());
    assert_eq!(document, before);
}

#[test]
fn every_invalid_edit_rolls_back_all_earlier_changes_and_version() {
    let original = Document::new(1, "unchanged".into()).unwrap();
    let invalid = [
        change(pos(0, 2), pos(0, 1), None, "bad"),
        change(pos(99, 0), pos(99, 0), None, "bad"),
        change(pos(0, 1), pos(0, 2), None, "bad"), // surrogate interior after first change
        change(pos(0, 0), pos(0, 2), Some(1), "bad"),
    ];
    for last in invalid {
        let mut document = original.clone();
        assert!(document.apply_changes(2, &[full("😀\r\n"), last]).is_err());
        assert_eq!(document, original);
    }
    // A rejected batch does not consume the version.
    let mut document = original;
    assert!(
        document
            .apply_changes(2, &[change(pos(1, 0), pos(1, 0), None, "bad")])
            .is_err()
    );
    document.apply_changes(2, &[full("accepted")]).unwrap();
}

#[test]
fn range_length_includes_each_cr_lf_and_astral_code_unit() {
    let mut document = Document::new(1, "😀\r\nx\r\ny".into()).unwrap();
    document
        .apply_changes(2, &[change(pos(0, 0), pos(2, 0), Some(7), "")])
        .unwrap();
    assert_eq!(document.text(), "y");
    let before = document.clone();
    assert_eq!(
        document.apply_changes(
            3,
            &[TextChange {
                range: None,
                range_length: Some(0),
                text: "replacement".into()
            }]
        ),
        Err(TextError::RangeLengthMismatch {
            expected: 0,
            actual: 1
        })
    );
    assert_eq!(document, before);
    document
        .apply_changes(
            3,
            &[TextChange {
                range: None,
                range_length: Some(1),
                text: "replacement".into(),
            }],
        )
        .unwrap();
}

#[test]
fn deletion_across_mixed_newlines_preserves_unaffected_bytes() {
    let mut document = Document::new(0, "a\r\n😀\rb\nc".into()).unwrap();
    document
        .apply_changes(1, &[change(pos(0, 1), pos(2, 1), Some(6), "\r")])
        .unwrap();
    assert_eq!(document.text(), "a\r\nc");
    assert!(offset_to_position(document.text(), 2).is_err());
}

#[test]
fn reversed_and_out_of_bounds_ranges_are_errors() {
    let mut document = Document::new(0, "x\ny".into()).unwrap();
    let before = document.clone();
    let reversed = Range {
        start: pos(1, 0),
        end: pos(0, 1),
    };
    assert_eq!(
        document.apply_changes(
            1,
            &[TextChange {
                range: Some(reversed),
                range_length: None,
                text: String::new()
            }]
        ),
        Err(TextError::ReversedRange(reversed))
    );
    assert_eq!(document, before);
    for invalid in [
        change(pos(0, 0), pos(0, 2), None, ""),
        change(pos(2, 0), pos(2, 0), None, ""),
    ] {
        assert!(document.apply_changes(1, &[invalid]).is_err());
        assert_eq!(document, before);
    }
}

#[test]
fn size_limit_is_in_bytes_and_checked_before_each_replacement() {
    let max = "😀".repeat(MAX_DOCUMENT_BYTES / 4);
    let mut document = Document::new(0, max.clone()).unwrap();
    assert!(matches!(
        Document::new(0, format!("{max}a")),
        Err(TextError::DocumentTooLarge { .. })
    ));
    let before = document.clone();
    assert!(matches!(
        document.apply_changes(1, &[change(pos(0, 0), pos(0, 0), None, "a")]),
        Err(TextError::DocumentTooLarge { .. })
    ));
    assert_eq!(document, before);
    document
        .apply_changes(1, &[change(pos(0, 0), pos(0, 2), Some(2), "😀")])
        .unwrap();
    assert_eq!(document.text(), max);
    let too_large = format!("{max}a");
    let before = document.clone();
    assert!(
        document
            .apply_changes(2, &[full("temporary"), full(&too_large), full("")])
            .is_err()
    );
    assert_eq!(document, before);
    assert!(position_to_offset(&too_large, pos(0, 0)).is_err());
    assert!(offset_to_position(&too_large, 0).is_err());
}

#[test]
fn change_count_limit_is_atomic() {
    let mut document = Document::new(0, String::new()).unwrap();
    let changes = vec![full("ok"); MAX_CHANGES];
    document.apply_changes(1, &changes).unwrap();
    let before = document.clone();
    let changes = vec![full("bad"); MAX_CHANGES + 1];
    assert_eq!(
        document.apply_changes(2, &changes),
        Err(TextError::TooManyChanges {
            count: MAX_CHANGES + 1,
            limit: MAX_CHANGES
        })
    );
    assert_eq!(document, before);
}

// Independent UTF-16 reference: split into lines before considering columns.
fn reference_positions(text: &str) -> Vec<(usize, Position)> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut line = 0;
    loop {
        let end = text.as_bytes()[start..]
            .iter()
            .position(|b| *b == b'\r' || *b == b'\n')
            .map_or(text.len(), |n| start + n);
        let mut units = 0;
        for (offset, c) in text[start..end].char_indices() {
            result.push((start + offset, pos(line, units)));
            units += c.encode_utf16(&mut [0; 2]).len() as u32;
        }
        result.push((end, pos(line, units)));
        if end == text.len() {
            break;
        }
        start = end
            + if text[end..].starts_with("\r\n") {
                2
            } else {
                1
            };
        line += 1;
    }
    result
}

#[test]
fn generated_utf8_newline_and_coordinate_crash_corpus() {
    let alphabet = ["a", "😀", "\u{301}", "水", "\r", "\n", "\0"];
    // Exhaustively test every sequence of up to five varied scalars.
    for length in 0..=5 {
        for mut index in 0..alphabet.len().pow(length) {
            let mut text = String::new();
            for _ in 0..length {
                text.push_str(alphabet[index % alphabet.len()]);
                index /= alphabet.len();
            }
            let reference = reference_positions(&text);
            for offset in 0..=text.len() + 1 {
                match reference.iter().find(|(byte, _)| *byte == offset) {
                    Some((_, position)) => {
                        assert_eq!(offset_to_position(&text, offset), Ok(*position), "{text:?}")
                    }
                    None => assert!(
                        offset_to_position(&text, offset).is_err(),
                        "{text:?}: {offset}"
                    ),
                }
            }
            for line in 0..=length + 1 {
                for column in 0..=2 * length + 1 {
                    let position = pos(line, column);
                    match reference.iter().find(|(_, p)| *p == position) {
                        Some((byte, _)) => {
                            assert_eq!(position_to_offset(&text, position), Ok(*byte), "{text:?}")
                        }
                        None => assert!(
                            position_to_offset(&text, position).is_err(),
                            "{text:?}: {position:?}"
                        ),
                    }
                }
            }
        }
    }
}

#[test]
fn generated_sequential_edits_match_byte_reference() {
    let alphabet = ["a", "😀", "\u{301}", "水", "\r", "\n", "\0"];
    let mut state = 0x1acb2d3_u64;
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (state >> 32) as usize
    };
    for _ in 0..512 {
        let mut expected = "😀\r\nx\ry\n".to_owned();
        let mut document = Document::new(0, expected.clone()).unwrap();
        let mut changes = Vec::new();
        for _ in 0..8 {
            let positions = reference_positions(&expected);
            let mut a = next() % positions.len();
            let mut b = next() % positions.len();
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            let (start, start_position) = positions[a];
            let (end, end_position) = positions[b];
            let replacement = alphabet[next() % alphabet.len()];
            changes.push(change(
                start_position,
                end_position,
                Some(expected[start..end].encode_utf16().count() as u32),
                replacement,
            ));
            expected.replace_range(start..end, replacement);
        }
        document.apply_changes(1, &changes).unwrap();
        assert_eq!(document.text(), expected);
        assert_eq!(document.version(), 1);
        let before = document.clone();
        changes.push(change(
            pos(u32::MAX, u32::MAX),
            pos(u32::MAX, u32::MAX),
            None,
            "bad",
        ));
        assert!(document.apply_changes(2, &changes).is_err());
        assert_eq!(document, before);
    }
}
