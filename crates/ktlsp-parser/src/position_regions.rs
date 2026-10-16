//! Original borrowed structural regions containing a UTF-8 byte position.
use crate::structural_regions::{Region, RegionDirectory};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionRegionError {
    OffsetOutOfBounds,
    NotCharBoundary,
}
impl std::fmt::Display for PositionRegionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "position regions rejected offset: {self:?}")
    }
}
impl std::error::Error for PositionRegionError {}

/// Return every original region whose half-open byte range contains `offset`,
/// in directory order. EOF is valid and yields no regions. Convert LSP UTF-16
/// positions using `ktlsp_text` on the identical snapshot before calling.
///
/// A complete iteration examines at most the accepted directory's 8,192 rows,
/// with constant query storage and no heap allocation; one `next()` may examine
/// every remaining row. This is structural containment, not Kotlin resolution.
/// Publication still requires a synchronized current-document recheck.
///
/// Returned references cannot escape the directory:
/// ```compile_fail
/// use ktlsp_parser::{SnapshotResult, lowering::LoweredDeclarations};
/// use ktlsp_parser::structural_regions::{Region, build_regions};
/// use ktlsp_parser::position_regions::containing_regions;
/// fn escape<'a>(lowered: &'a LoweredDeclarations<'a>, result: &'a SnapshotResult)
///     -> &'a Region<'a>
/// {
///     let directory = build_regions(lowered, result, result.snapshot()).unwrap();
///     containing_regions(&directory, 0).unwrap().next().unwrap()
/// }
/// ```
pub fn containing_regions<'q, 'a: 'q>(
    directory: &'q RegionDirectory<'a>,
    offset: usize,
) -> Result<impl Iterator<Item = &'q Region<'a>> + 'q, PositionRegionError> {
    let source = directory.snapshot().source();
    if offset > source.len() {
        return Err(PositionRegionError::OffsetOutOfBounds);
    }
    if !source.is_char_boundary(offset) {
        return Err(PositionRegionError::NotCharBoundary);
    }
    Ok(directory.regions().iter().filter(move |region| {
        let range = region.range();
        range.start <= offset && offset < range.end
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Element, FileKind, Parsed, SnapshotResult, SourceSnapshot};
    use crate::{lowering::lower_declarations, structural_regions::build_regions};
    use std::sync::Arc;

    #[test]
    fn accepted_empty_root_matches_nothing() {
        let source: Arc<str> = "".into();
        let result = SnapshotResult {
            snapshot: SourceSnapshot::new("position-empty", 1, FileKind::Source, source.clone())
                .unwrap(),
            parsed: Parsed {
                source,
                elements: vec![Element {
                    kind: "KT_FILE".into(),
                    start: 0,
                    end: 0,
                    parent: None,
                    subtree_end: 1,
                    token: false,
                }],
                diagnostics: vec![],
            },
        };
        let lowered = lower_declarations(&result, result.snapshot()).unwrap();
        let directory = build_regions(&lowered, &result, result.snapshot()).unwrap();
        assert_eq!(directory.regions().len(), 1);
        assert_eq!(directory.regions()[0].range(), 0..0);
        assert!(containing_regions(&directory, 0).unwrap().next().is_none());
        assert_eq!(
            containing_regions(&directory, 1).err(),
            Some(PositionRegionError::OffsetOutOfBounds)
        );
    }
}
