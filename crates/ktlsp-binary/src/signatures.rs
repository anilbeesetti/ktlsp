//! JVMS 23 §4.7.9.1 signatures. Names borrow lossless UTF-16; no linkage/erasure checks.
use crate::{Attribute, ClassFile, Error};

type Result<T> = std::result::Result<T, Error>;
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_NODES: usize = 65_535;
pub const MAX_RECORDS: usize = 65_535;
pub const MAX_WORK: usize = 16 * 1024 * 1024;
pub const MAX_DEPTH: usize = 32;

/// Caller limits may reduce, but never raise, the hard caps.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
    pub nodes: usize,
    pub records: usize,
    pub work: usize,
    pub depth: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: MAX_BYTES,
            nodes: MAX_NODES,
            records: MAX_RECORDS,
            work: MAX_WORK,
            depth: MAX_DEPTH,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Context {
    Class,
    Field,
    Method,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Type<'a> {
    /// One of B C D F I J S Z, never V.
    Base(u16),
    Class(ClassType<'a>),
    Variable(&'a [u16]),
    /// Consecutive dimensions are flattened to avoid recursive array traversal/drop.
    Array {
        dimensions: u8,
        component: Box<Type<'a>>,
    },
}
#[derive(Debug, PartialEq, Eq)]
pub struct ClassType<'a> {
    pub package: Vec<&'a [u16]>,
    /// First class followed by nested owner suffixes, each with its own arguments.
    pub segments: Vec<Segment<'a>>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Segment<'a> {
    pub name: &'a [u16],
    pub arguments: Vec<Argument<'a>>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Argument<'a> {
    Any,
    Exact(Type<'a>),
    Extends(Type<'a>),
    Super(Type<'a>),
}
#[derive(Debug, PartialEq, Eq)]
pub struct Parameter<'a> {
    pub name: &'a [u16],
    pub class_bound: Option<Type<'a>>,
    pub interface_bounds: Vec<Type<'a>>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ClassSignature<'a> {
    pub parameters: Vec<Parameter<'a>>,
    pub superclass: ClassType<'a>,
    pub interfaces: Vec<ClassType<'a>>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct MethodSignature<'a> {
    pub parameters: Vec<Parameter<'a>>,
    pub arguments: Vec<Type<'a>>,
    /// None denotes void.
    pub result: Option<Type<'a>>,
    /// Only Class and Variable variants occur.
    pub throws: Vec<Type<'a>>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Signature<'a> {
    Class(ClassSignature<'a>),
    Field(Type<'a>),
    Method(MethodSignature<'a>),
}
#[derive(Debug, PartialEq, Eq)]
pub struct Signatures<'a> {
    pub class: Option<Signature<'a>>,
    /// Aligned with the original member order, including absent attributes.
    pub fields: Vec<Option<Signature<'a>>>,
    pub methods: Vec<Option<Signature<'a>>>,
}

struct Budget {
    left: Limits,
}
fn charge(left: &mut usize, count: usize, name: &'static str) -> Result<()> {
    *left = left.checked_sub(count).ok_or(Error::Limit(name))?;
    Ok(())
}
impl Budget {
    fn new(l: Limits) -> Self {
        Self {
            left: Limits {
                bytes: l.bytes.min(MAX_BYTES),
                nodes: l.nodes.min(MAX_NODES),
                records: l.records.min(MAX_RECORDS),
                work: l.work.min(MAX_WORK),
                depth: l.depth.min(MAX_DEPTH),
            },
        }
    }
    fn node(&mut self) -> Result<()> {
        charge(&mut self.left.nodes, 1, "signature nodes")
    }
    fn record(&mut self) -> Result<()> {
        charge(&mut self.left.records, 1, "signature records")
    }
    fn bytes(&mut self, n: usize) -> Result<()> {
        charge(&mut self.left.bytes, n, "signature bytes")
    }
    fn work(&mut self, n: usize) -> Result<()> {
        charge(&mut self.left.work, n, "signature work")
    }
    fn depth(&self, depth: usize) -> Result<()> {
        if depth > self.left.depth {
            Err(Error::Limit("signature depth"))
        } else {
            Ok(())
        }
    }
}
struct Parser<'a, 'b> {
    text: &'a [u16],
    at: usize,
    budget: &'b mut Budget,
}
fn delimiter(c: u16) -> bool {
    matches!(c, 46 | 59 | 91 | 47 | 60 | 62 | 58)
}
impl<'a> Parser<'a, '_> {
    fn peek(&self) -> Option<u16> {
        self.text.get(self.at).copied()
    }
    fn eat(&mut self, c: u16) -> bool {
        if self.peek() == Some(c) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, c: u16) -> Result<()> {
        if self.eat(c) {
            Ok(())
        } else {
            Err(Error::Malformed("signature syntax"))
        }
    }
    fn identifier(&mut self) -> Result<&'a [u16]> {
        self.budget.node()?;
        let start = self.at;
        while self.peek().is_some_and(|c| !delimiter(c)) {
            self.at += 1;
        }
        if start == self.at {
            return Err(Error::Malformed("signature identifier"));
        }
        Ok(&self.text[start..self.at])
    }
    fn java_type(&mut self, depth: usize) -> Result<Type<'a>> {
        self.budget.depth(depth)?;
        if let Some(c @ (66 | 67 | 68 | 70 | 73 | 74 | 83 | 90)) = self.peek() {
            self.budget.node()?;
            self.at += 1;
            Ok(Type::Base(c))
        } else {
            self.reference(depth)
        }
    }
    fn reference(&mut self, depth: usize) -> Result<Type<'a>> {
        self.budget.depth(depth)?;
        match self.peek() {
            Some(76) => Ok(Type::Class(self.class_type(depth)?)),
            Some(84) => {
                self.budget.node()?;
                self.at += 1;
                let name = self.identifier()?;
                self.expect(59)?;
                Ok(Type::Variable(name))
            }
            Some(91) => {
                self.budget.node()?;
                let mut dimensions = 0u8;
                while self.eat(91) {
                    dimensions = dimensions
                        .checked_add(1)
                        .ok_or(Error::Limit("signature array dimensions"))?;
                }
                let component = self.java_type(depth + 1)?;
                Ok(Type::Array {
                    dimensions,
                    component: Box::new(component),
                })
            }
            _ => Err(Error::Malformed("signature reference type")),
        }
    }
    fn class_type(&mut self, depth: usize) -> Result<ClassType<'a>> {
        self.budget.depth(depth)?;
        self.budget.node()?;
        self.expect(76)?;
        let mut package = Vec::new();
        let mut name = self.identifier()?;
        while self.eat(47) {
            package.push(name);
            name = self.identifier()?;
        }
        let mut segments = vec![self.segment(name, depth)?];
        while self.eat(46) {
            let name = self.identifier()?;
            segments.push(self.segment(name, depth)?);
        }
        self.expect(59)?;
        Ok(ClassType { package, segments })
    }
    fn segment(&mut self, name: &'a [u16], depth: usize) -> Result<Segment<'a>> {
        self.budget.node()?;
        let mut arguments = Vec::new();
        if self.eat(60) {
            loop {
                self.budget.node()?;
                let arg = if self.eat(42) {
                    Argument::Any
                } else if self.eat(43) {
                    Argument::Extends(self.reference(depth + 1)?)
                } else if self.eat(45) {
                    Argument::Super(self.reference(depth + 1)?)
                } else {
                    Argument::Exact(self.reference(depth + 1)?)
                };
                arguments.push(arg);
                if self.eat(62) {
                    break;
                }
            }
        }
        Ok(Segment { name, arguments })
    }
    fn bound_present(&mut self) -> Result<bool> {
        match self.peek() {
            Some(91) => Ok(true),
            Some(76 | 84) => {
                // An absent bound may be followed by another parameter named Long or T.
                let mut end = self.at + 1;
                while let Some(&c) = self.text.get(end) {
                    self.budget.work(1)?;
                    if delimiter(c) {
                        return Ok(c != 58);
                    }
                    end += 1;
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    fn parameters(&mut self) -> Result<Vec<Parameter<'a>>> {
        let mut out = Vec::new();
        if self.eat(60) {
            loop {
                self.budget.node()?;
                let name = self.identifier()?;
                self.expect(58)?;
                let class_bound = if self.bound_present()? {
                    Some(self.reference(1)?)
                } else {
                    None
                };
                let mut interface_bounds = Vec::new();
                while self.eat(58) {
                    interface_bounds.push(self.reference(1)?);
                }
                out.push(Parameter {
                    name,
                    class_bound,
                    interface_bounds,
                });
                if self.eat(62) {
                    break;
                }
            }
        }
        Ok(out)
    }
    fn signature(&mut self, context: Context) -> Result<Signature<'a>> {
        self.budget.node()?;
        let out = match context {
            Context::Field => Signature::Field(self.reference(1)?),
            Context::Class => {
                let parameters = self.parameters()?;
                let superclass = self.class_type(1)?;
                let mut interfaces = Vec::new();
                while self.peek().is_some() {
                    interfaces.push(self.class_type(1)?);
                }
                Signature::Class(ClassSignature {
                    parameters,
                    superclass,
                    interfaces,
                })
            }
            Context::Method => {
                let parameters = self.parameters()?;
                self.expect(40)?;
                let mut arguments = Vec::new();
                while !self.eat(41) {
                    arguments.push(self.java_type(1)?);
                }
                let result = if self.eat(86) {
                    None
                } else {
                    Some(self.java_type(1)?)
                };
                let mut throws = Vec::new();
                while self.eat(94) {
                    if !matches!(self.peek(), Some(76 | 84)) {
                        return Err(Error::Malformed("signature throws"));
                    }
                    throws.push(self.reference(1)?);
                }
                Signature::Method(MethodSignature {
                    parameters,
                    arguments,
                    result,
                    throws,
                })
            }
        };
        if self.at != self.text.len() {
            return Err(Error::Malformed("signature trailing data"));
        }
        Ok(out)
    }
}
fn parse<'a>(text: &'a [u16], context: Context, budget: &mut Budget) -> Result<Signature<'a>> {
    budget.bytes(
        text.len()
            .checked_mul(2)
            .ok_or(Error::Limit("signature bytes"))?,
    )?;
    budget.work(text.len())?;
    Parser {
        text,
        at: 0,
        budget,
    }
    .signature(context)
}
pub fn parse_signature(text: &[u16], context: Context) -> Result<Signature<'_>> {
    parse_signature_with_limits(text, context, Limits::default())
}
pub fn parse_signature_with_limits(
    text: &[u16],
    context: Context,
    limits: Limits,
) -> Result<Signature<'_>> {
    parse(text, context, &mut Budget::new(limits))
}
fn utf16<'a>(class: &'a ClassFile<'_>, index: u16) -> Result<&'a [u16]> {
    if index == 0 {
        return Err(Error::Malformed("signature constant index"));
    }
    class.utf16(index)
}
fn attribute<'a>(
    class: &'a ClassFile<'_>,
    attrs: &[Attribute<'_>],
    context: Context,
    budget: &mut Budget,
) -> Result<Option<Signature<'a>>> {
    let mut out = None;
    for a in attrs {
        budget.record()?;
        budget.bytes(a.bytes.len())?;
        let name = utf16(class, a.name)?;
        budget.work(name.len())?;
        if !name.iter().copied().eq("Signature".encode_utf16()) {
            continue;
        }
        if class.major < 49 {
            return Err(Error::Malformed("signature attribute version"));
        }
        if out.is_some() {
            return Err(Error::Malformed("duplicate Signature"));
        }
        let [hi, lo] = *a.bytes else {
            return Err(Error::Malformed("Signature length"));
        };
        let text = utf16(class, u16::from_be_bytes([hi, lo]))?;
        out = Some(parse(text, context, budget)?);
    }
    Ok(out)
}
/// Decode class/field/method tables only. Raw attributes are untouched. Does not
/// validate the rest of a publicly constructed model or decode Record/Code interiors.
pub fn decode_signatures<'a>(class: &'a ClassFile<'_>) -> Result<Signatures<'a>> {
    decode_signatures_with_limits(class, Limits::default())
}
pub fn decode_signatures_with_limits<'a>(
    class: &'a ClassFile<'_>,
    limits: Limits,
) -> Result<Signatures<'a>> {
    if !(class.major == 45 && class.minor <= 3
        || (46..=67).contains(&class.major) && class.minor == 0)
    {
        return Err(Error::UnsupportedVersion {
            major: class.major,
            minor: class.minor,
        });
    }
    let mut budget = Budget::new(limits);
    budget.record()?;
    let signature = attribute(class, &class.attributes, Context::Class, &mut budget)?;
    let mut fields = Vec::new();
    for m in &class.fields {
        budget.record()?;
        fields.push(attribute(
            class,
            &m.attributes,
            Context::Field,
            &mut budget,
        )?);
    }
    let mut methods = Vec::new();
    for m in &class.methods {
        budget.record()?;
        methods.push(attribute(
            class,
            &m.attributes,
            Context::Method,
            &mut budget,
        )?);
    }
    Ok(Signatures {
        class: signature,
        fields,
        methods,
    })
}
