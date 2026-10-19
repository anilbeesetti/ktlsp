//! Kotlin metadata decoding, semantic declarations, and stable declaration identities.

pub(crate) mod contract_decoder;
pub(crate) mod decode;
pub mod id_signature;
pub mod semantic;
mod value_parameter_flags;
pub use value_parameter_flags::DeclaredValueParameter;

/// Canonical function flag layout shared with the original Kotlin metadata decoder.
pub(crate) mod function_flags {
    /// Visibility occupies bits 1-3; PRIVATE is 1.
    pub const VISIBILITY_MASK: u64 = 0b1110;
    pub const VISIBILITY_PRIVATE: u64 = 1 << 1;
    pub const IS_INLINE: u64 = 1 << 10;
    /// An `external` declaration: its implementation is supplied by the target, never by a body.
    pub const IS_EXTERNAL: u64 = 1 << 12;
    pub const IS_SUSPEND: u64 = 1 << 13;
    pub const IS_EXPECT: u64 = 1 << 14;
    /// Low bit of the 2-bit `ReturnValueStatus` field (bits 16-17), which follows
    /// `hasNonStableParameterNames` (bit 15).
    pub const RETURN_VALUE_STATUS_SHIFT: u32 = 16;
    /// The schema's `isStatic` bit.
    pub const IS_STATIC: u64 = 1 << 18;
    /// Bits 6-7 are `MemberKind`. DELEGATION (2) is a function realized by `interface by`.
    pub const MEMBER_KIND_DELEGATION: u64 = 2 << 6;
}

pub(crate) mod property_flags {
    /// Public, final property with a default getter; protobuf omits field 11 at this value.
    pub const DEFAULT: u64 = 518;
    pub const VISIBILITY_MASK: u64 = 0b1110;
    pub const IS_VAR: u64 = 1 << 8;
    pub const HAS_SETTER: u64 = 1 << 10;
    pub const IS_CONST: u64 = 1 << 11;
    pub const HAS_CONSTANT: u64 = 1 << 13;
    pub const MODALITY_OPEN: u64 = 1 << 4;
    /// Low bit of the 2-bit `ReturnValueStatus` field (bits 17-18).
    pub const RETURN_VALUE_STATUS_SHIFT: u32 = 17;
    pub const MODALITY_ABSTRACT: u64 = 1 << 5;
    /// `MEMBER_KIND` (bits 6-7) DELEGATION: a forwarder to an interface delegate.
    pub const MEMBER_KIND_DELEGATION: u64 = 2 << 6;
    pub const IS_LATEINIT: u64 = 1 << 12;
    pub const IS_DELEGATED: u64 = 1 << 15;
    pub const IS_EXPECT: u64 = 1 << 16;
    /// Accessor flag word bit 6: the accessor is not the compiler default.
    pub const ACCESSOR_IS_NOT_DEFAULT: u64 = 1 << 6;
    /// Accessor flag word bit 8: the accessor is declared `inline`.
    pub const ACCESSOR_IS_INLINE: u64 = 1 << 8;
    /// Both modality bits (4-5). Shared by the property word and the accessor word, which is how an
    /// accessor's default is derived from its property.
    pub const MODALITY_MASK: u64 = 0b11_0000;
    /// The schema's `isStatic` bit.
    pub const IS_STATIC: u64 = 1 << 19;
}
