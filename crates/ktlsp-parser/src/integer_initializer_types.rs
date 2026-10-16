//! Positive, untyped, direct local integer initializers; not a Kotlin type engine.
use crate::{
    Element, SourceSnapshot,
    local_value_binding::LocalValueBinding,
    lowering::{ReferenceBudget, ReferenceSiteError},
};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerType {
    Int,
    Long,
    UInt,
    ULong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerInitializerError {
    InvalidOrigin,
    StaleSnapshot,
    UnsupportedContext,
    ReferenceInitializer,
    InvalidLiteral,
    OutOfRange,
    Resource,
}
impl std::fmt::Display for IntegerInitializerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "integer initializer rejected request: {self:?}")
    }
}
impl std::error::Error for IntegerInitializerError {}
impl From<ReferenceSiteError> for IntegerInitializerError {
    fn from(error: ReferenceSiteError) -> Self {
        match error {
            ReferenceSiteError::InvalidOrigin => Self::InvalidOrigin,
            ReferenceSiteError::StaleSnapshot => Self::StaleSnapshot,
            ReferenceSiteError::UnsupportedContext => Self::UnsupportedContext,
            ReferenceSiteError::Resource => Self::Resource,
        }
    }
}
type Result<T, E = IntegerInitializerError> = std::result::Result<T, E>;

/// Retains the original binding and literal token, borrowing original source bytes.
#[derive(Debug)]
pub struct IntegerInitializer<'a> {
    binding: &'a LocalValueBinding<'a>,
    literal: &'a Element,
    raw: &'a str,
    value: u64,
    default_type: IntegerType,
}
impl<'a> IntegerInitializer<'a> {
    pub fn binding(&self) -> &'a LocalValueBinding<'a> {
        self.binding
    }
    pub fn literal(&self) -> &'a Element {
        self.literal
    }
    pub fn range(&self) -> Range<usize> {
        self.literal.start..self.literal.end
    }
    pub fn raw_spelling(&self) -> &'a str {
        self.raw
    }
    pub fn value(&self) -> u64 {
        self.value
    }
    pub fn default_type(&self) -> IntegerType {
        self.default_type
    }
}

/// Authenticate the original tree, then inspect only the already-validated target.
/// No initializer chasing, expected type, unary sign or constant folding is applied.
/// Synchronize the current document incarnation before publishing this result.
pub fn infer_integer_initializer<'a>(
    binding: &'a LocalValueBinding<'a>,
    current: &SourceSnapshot,
) -> Result<IntegerInitializer<'a>> {
    infer_with_budget(binding, current, &mut ReferenceBudget::default())
}
fn infer_with_budget<'a>(
    binding: &'a LocalValueBinding<'a>,
    current: &SourceSnapshot,
    budget: &mut ReferenceBudget,
) -> Result<IntegerInitializer<'a>> {
    let tree = binding.site().tree(current, budget)?;
    let target_index = binding.target().cst_index();
    let target = budget.element(tree, target_index)?;
    if !budget.equal(&target.kind, "PROPERTY")? {
        return Err(IntegerInitializerError::UnsupportedContext);
    }
    let mut cursor = target_index
        .checked_add(1)
        .ok_or(IntegerInitializerError::Resource)?;
    let mut after_eq = false;
    while cursor < target.subtree_end {
        let node = budget.element(tree, cursor)?;
        if node.parent != Some(target_index) {
            return Err(IntegerInitializerError::UnsupportedContext);
        }
        if !after_eq && (!node.token || budget.equal(&node.kind, "COLON")?) {
            return Err(IntegerInitializerError::UnsupportedContext);
        }
        if after_eq && !node.token {
            if budget.equal(&node.kind, "REFERENCE_EXPRESSION")? {
                return Err(IntegerInitializerError::ReferenceInitializer);
            }
            if !budget.equal(&node.kind, "INTEGER_CONSTANT")? {
                return Err(IntegerInitializerError::UnsupportedContext);
            }
            let token_index = cursor
                .checked_add(1)
                .ok_or(IntegerInitializerError::Resource)?;
            let literal = budget.element(tree, token_index)?;
            if !literal.token
                || literal.parent != Some(cursor)
                || !budget.equal(&literal.kind, "INTEGER_LITERAL")?
            {
                return Err(IntegerInitializerError::UnsupportedContext);
            }
            let raw = tree
                .source()
                .get(literal.start..literal.end)
                .ok_or(IntegerInitializerError::UnsupportedContext)?;
            let (value, default_type) = decode(raw, budget)?;
            return Ok(IntegerInitializer {
                binding,
                literal,
                raw,
                value,
                default_type,
            });
        }
        if node.token && budget.equal(&node.kind, "EQ")? {
            after_eq = true;
        }
        cursor = node.subtree_end;
    }
    Err(IntegerInitializerError::UnsupportedContext)
}

// Independently implemented from the pinned language/compiler rules in README.
// Scan borrowed bytes directly: no underscore removal, allocation or recursion.
fn decode(raw: &str, budget: &mut ReferenceBudget) -> Result<(u64, IntegerType)> {
    budget.charge(raw.len())?;
    let mut digits = raw.as_bytes();
    let long = digits.last() == Some(&b'L');
    if long {
        digits = &digits[..digits.len() - 1];
    }
    let unsigned = matches!(digits.last(), Some(b'u' | b'U'));
    if unsigned {
        digits = &digits[..digits.len() - 1];
    }
    let radix = match digits {
        [b'0', b'x' | b'X', ..] => 16u64,
        [b'0', b'b' | b'B', ..] => 2,
        _ => 10,
    };
    if radix != 10 {
        digits = &digits[2..];
    } else if digits.len() > 1 && digits.first() == Some(&b'0') {
        return Err(IntegerInitializerError::InvalidLiteral);
    }
    if digits.is_empty() || digits.first() == Some(&b'_') || digits.last() == Some(&b'_') {
        return Err(IntegerInitializerError::InvalidLiteral);
    }
    let mut value = 0u64;
    for &byte in digits {
        if byte == b'_' {
            continue;
        }
        let digit = match byte {
            b'0'..=b'9' => u64::from(byte - b'0'),
            b'a'..=b'f' => u64::from(byte - b'a' + 10),
            b'A'..=b'F' => u64::from(byte - b'A' + 10),
            _ => return Err(IntegerInitializerError::InvalidLiteral),
        };
        if digit >= radix {
            return Err(IntegerInitializerError::InvalidLiteral);
        }
        value = value
            .checked_mul(radix)
            .and_then(|v| v.checked_add(digit))
            .ok_or(IntegerInitializerError::OutOfRange)?;
    }
    let ty = if unsigned {
        if long || value > u64::from(u32::MAX) {
            IntegerType::ULong
        } else {
            IntegerType::UInt
        }
    } else {
        if value > i64::MAX as u64 {
            return Err(IntegerInitializerError::OutOfRange);
        }
        if long || value > i32::MAX as u64 {
            IntegerType::Long
        } else {
            IntegerType::Int
        }
    };
    Ok((value, ty))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_literal_boundaries_and_spellings() {
        use IntegerType::*;
        for (raw, value, ty) in [
            ("0", 0, Int),
            ("2147483647", 2147483647, Int),
            ("2147483648", 2147483648, Long),
            ("9223372036854775807", i64::MAX as u64, Long),
            ("1L", 1, Long),
            ("0u", 0, UInt),
            ("1U", 1, UInt),
            ("4294967295u", u32::MAX as u64, UInt),
            ("4294967296U", 4294967296, ULong),
            ("18446744073709551615u", u64::MAX, ULong),
            ("0uL", 0, ULong),
            ("1UL", 1, ULong),
            ("0x7fffFFFF", i32::MAX as u64, Int),
            ("0X80000000", 2147483648, Long),
            ("0x7fff_ffff_ffff_ffffL", i64::MAX as u64, Long),
            ("0XFFFF_FFFFu", u32::MAX as u64, UInt),
            ("0xffff_ffff_ffff_ffffUL", u64::MAX, ULong),
            ("0b1__0", 2, Int),
            ("0B1111U", 15, UInt),
            ("1__000", 1000, Int),
            ("0x00", 0, Int),
        ] {
            assert_eq!(
                decode(raw, &mut ReferenceBudget::default()),
                Ok((value, ty)),
                "{raw}"
            );
        }
        for raw in [
            "9223372036854775808",
            "9223372036854775808L",
            "0x8000000000000000",
            "18446744073709551616u",
            "0x10000000000000000UL",
            "0b10000000000000000000000000000000000000000000000000000000000000000u",
        ] {
            assert_eq!(
                decode(raw, &mut ReferenceBudget::default()),
                Err(IntegerInitializerError::OutOfRange),
                "{raw}"
            );
        }
        for raw in [
            "", "_1", "1_", "1_u", "1_UL", "0x_1", "0x1_", "0b_1", "0b2", "0b", "0x", "00", "01",
            "0_0", "1l", "1ul", "1Ul", "1LU", "1uu", "+1", "-1", "1.0", "1e2", "1f", "1 u", "１",
            "0o7",
        ] {
            assert_eq!(
                decode(raw, &mut ReferenceBudget::default()),
                Err(IntegerInitializerError::InvalidLiteral),
                "{raw}"
            );
        }
    }

    #[test]
    fn decoder_charges_raw_bytes_before_scanning() {
        let mut budget = ReferenceBudget::default();
        budget.charge(2_097_152).unwrap();
        assert_eq!(
            decode("1", &mut budget),
            Err(IntegerInitializerError::Resource)
        );
        let mut budget = ReferenceBudget::default();
        budget.charge(2_097_151).unwrap();
        assert_eq!(decode("1", &mut budget), Ok((1, IntegerType::Int)));
    }
}
