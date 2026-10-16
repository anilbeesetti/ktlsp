//! Borrowed JVMS §4.3 descriptor structure, not generic signatures or resolved types.
use crate::{Error, Result, internal_name};

/// Maximum UTF-16 units per supplied descriptor (the classfile UTF8 length ceiling).
/// Validation is iterative and linear, with at most two scans of object names.
pub const MAX_UNITS: usize = 65_535;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Primitive {
    Byte,
    Char,
    Double,
    Float,
    Int,
    Long,
    Short,
    Boolean,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Component<'a> {
    Primitive(Primitive),
    /// Lossless internal name, excluding L and ;. No normalization or resolution.
    Object(&'a [u16]),
}

/// A validated field type, also used for parameters and non-void returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldDescriptor<'a> {
    raw: &'a [u16],
    dimensions: u8,
    component: Component<'a>,
}
impl<'a> FieldDescriptor<'a> {
    pub fn parse(raw: &'a [u16]) -> Result<Self> {
        input_limit(raw)?;
        let mut at = 0;
        let field = field_type(raw, &mut at)?;
        complete(raw, at)?;
        Ok(field)
    }
    pub fn raw(self) -> &'a [u16] {
        self.raw
    }
    /// Flattened array rank, or zero for a non-array type.
    pub fn dimensions(self) -> u8 {
        self.dimensions
    }
    pub fn component(self) -> Component<'a> {
        self.component
    }
    /// Arrays and references occupy one slot; non-array long/double occupy two.
    pub fn slots(self) -> u8 {
        if self.dimensions == 0
            && matches!(
                self.component,
                Component::Primitive(Primitive::Long | Primitive::Double)
            )
        {
            2
        } else {
            1
        }
    }
}

/// Whether the 255-slot parameter limit must include an implicit receiver.
/// The receiver is not encoded in the descriptor and is never yielded as a parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotContext {
    WithoutReceiver,
    WithReceiver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MethodDescriptor<'a> {
    raw: &'a [u16],
    parameters: &'a [u16],
    result: Option<FieldDescriptor<'a>>,
    slots: u8,
}
impl<'a> MethodDescriptor<'a> {
    /// Validates the entire input, including all parameters and the return type.
    pub fn parse(raw: &'a [u16], context: SlotContext) -> Result<Self> {
        input_limit(raw)?;
        if raw.first() != Some(&40) {
            return Err(Error::Malformed("method descriptor"));
        }
        let mut at = 1;
        let mut slots = usize::from(context == SlotContext::WithReceiver);
        while raw.get(at).is_some_and(|c| *c != 41) {
            slots = slots
                .checked_add(usize::from(field_type(raw, &mut at)?.slots()))
                .ok_or(Error::Malformed("parameter slots"))?;
            if slots > 255 {
                return Err(Error::Malformed("parameter slots"));
            }
        }
        if raw.get(at) != Some(&41) {
            return Err(Error::Malformed("method descriptor"));
        }
        let parameters = &raw[1..at];
        at += 1;
        let result = if raw.get(at) == Some(&86) {
            at += 1;
            None
        } else {
            Some(field_type(raw, &mut at)?)
        };
        complete(raw, at)?;
        Ok(Self {
            raw,
            parameters,
            result,
            slots: slots as u8,
        })
    }
    pub fn raw(self) -> &'a [u16] {
        self.raw
    }
    /// A fresh allocation-free iterator, in encoded parameter order.
    pub fn parameters(self) -> Parameters<'a> {
        Parameters {
            remaining: self.parameters,
        }
    }
    /// None denotes void, which is never a field or parameter type.
    pub fn result(self) -> Option<FieldDescriptor<'a>> {
        self.result
    }
    /// Total slots, including the receiver exactly when requested at parse time.
    pub fn parameter_slots(self) -> u8 {
        self.slots
    }
}

#[derive(Clone, Debug)]
pub struct Parameters<'a> {
    remaining: &'a [u16],
}
impl<'a> Iterator for Parameters<'a> {
    type Item = FieldDescriptor<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }
        let mut at = 0;
        // Only complete, validated method parameter slices can construct this iterator.
        let field = field_type(self.remaining, &mut at).ok()?;
        self.remaining = &self.remaining[at..];
        Some(field)
    }
}
impl std::iter::FusedIterator for Parameters<'_> {}

fn input_limit(raw: &[u16]) -> Result<()> {
    if raw.len() > MAX_UNITS {
        return Err(Error::Limit("descriptor units"));
    }
    Ok(())
}
fn complete(raw: &[u16], at: usize) -> Result<()> {
    if at != raw.len() {
        return Err(Error::Malformed("descriptor trailing data"));
    }
    Ok(())
}

// The sole field-type grammar: classfile validation, views and iteration all use it.
fn field_type<'a>(s: &'a [u16], at: &mut usize) -> Result<FieldDescriptor<'a>> {
    let start = *at;
    let mut dimensions = 0u8;
    while s.get(*at) == Some(&91) {
        dimensions = dimensions
            .checked_add(1)
            .ok_or(Error::Malformed("array depth"))?;
        *at += 1;
    }
    let c = *s.get(*at).ok_or(Error::Malformed("descriptor"))?;
    *at += 1;
    let primitive = match c {
        66 => Primitive::Byte,
        67 => Primitive::Char,
        68 => Primitive::Double,
        70 => Primitive::Float,
        73 => Primitive::Int,
        74 => Primitive::Long,
        83 => Primitive::Short,
        90 => Primitive::Boolean,
        76 => {
            let begin = *at;
            while s.get(*at).is_some_and(|c| *c != 59) {
                *at += 1;
            }
            if s.get(*at) != Some(&59) {
                return Err(Error::Malformed("object descriptor"));
            }
            let name = &s[begin..*at];
            internal_name(name)?;
            *at += 1;
            return Ok(FieldDescriptor {
                raw: &s[start..*at],
                dimensions,
                component: Component::Object(name),
            });
        }
        _ => return Err(Error::Malformed("descriptor type")),
    };
    Ok(FieldDescriptor {
        raw: &s[start..*at],
        dimensions,
        component: Component::Primitive(primitive),
    })
}
