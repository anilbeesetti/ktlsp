//! Original JPS XML declared roots only, not effective membership or a classpath.
//! No filesystem, SDK/global-variable discovery, or build-tool execution occurs.

use ktlsp_project::{ProjectModel, SourceRootKind};
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use serde::Serialize;
use std::{
    borrow::Cow,
    fmt, io,
    mem::size_of,
    ops::Range,
    path::{Component, Path, PathBuf},
};

pub const MAX_DOCUMENTS: usize = 128;
pub const MAX_DOCUMENT_BYTES: usize = 64 * 1024;
pub const MAX_TOTAL_BYTES: usize = 1024 * 1024;
pub const MAX_EVENTS: usize = 8192;
pub const MAX_DEPTH: usize = 32;
pub const MAX_ATTRIBUTES: usize = 16;
pub const MAX_PATH_BYTES: usize = 4096;
pub const MAX_TOTAL_PATH_BYTES: usize = 1024 * 1024;
pub const MAX_STORAGE_BYTES: usize = ktlsp_project::MAX_CALLER_PATH_BYTES;
pub const MAX_WORK: usize = crate::MAX_NODES;

/// Snapshot key: project-relative or absolute host path, never a URL or macro.
pub struct Document<'a> {
    pub path: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Debug)]
pub enum Error {
    Budget(&'static str),
    Invalid {
        document: String,
        offset: usize,
        reason: &'static str,
    },
    Unsupported {
        document: String,
        offset: usize,
        reason: &'static str,
    },
    Project(ktlsp_project::ProjectError),
    Serialization(serde_json::Error),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Budget(b) => write!(f, "JPS {b} budget exceeded"),
            Self::Invalid {
                document,
                offset,
                reason,
            } => write!(f, "invalid JPS {document} at {offset}: {reason}"),
            Self::Unsupported {
                document,
                offset,
                reason,
            } => write!(f, "unsupported JPS {document} at {offset}: {reason}"),
            Self::Project(e) => write!(f, "invalid JPS declared-source projection: {e}"),
            Self::Serialization(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Project(e) => Some(e),
            Self::Serialization(e) => Some(e),
            _ => None,
        }
    }
}
fn invalid(doc: &str, offset: usize, reason: &'static str) -> Error {
    Error::Invalid {
        document: doc.into(),
        offset,
        reason,
    }
}
fn unsupported(doc: &str, offset: usize, reason: &'static str) -> Error {
    Error::Unsupported {
        document: doc.into(),
        offset,
        reason,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Source {
    /// Normalized snapshot document path; no physical identity is implied.
    pub document: PathBuf,
    /// Complete original element bytes, including any UTF-8 BOM displacement.
    pub range: Range<usize>,
}
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SourceRoot {
    pub path: PathBuf,
    pub kind: SourceRootKind,
    pub generated: bool,
    #[serde(skip)]
    pub source: Source,
}
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Module {
    pub id: String,
    #[serde(rename = "sourceRoots")]
    pub source_roots: Vec<SourceRoot>,
    /// Original .iml module element.
    #[serde(skip)]
    pub source: Source,
    /// Original modules.xml module reference.
    #[serde(skip)]
    pub declaration: Source,
}
/// Immutable declared-source records with their original XML provenance.
/// ```compile_fail
/// fn mutate(mut w: ktlsp_import::jps::ImportedSources) { w.modules.clear(); }
/// ```
#[derive(Debug)]
pub struct ImportedSources {
    project: ProjectModel,
    modules: Vec<Module>,
}
impl ImportedSources {
    /// Original modules and declared roots in document order.
    pub fn modules(&self) -> &[Module] {
        &self.modules
    }
    /// Declared roots ONLY. Empty dependency collections do not describe a classpath.
    pub fn declared_source_projection(&self) -> &ProjectModel {
        &self.project
    }
}

#[derive(Default)]
struct Budget {
    bytes: usize,
    events: usize,
    work: usize,
    storage: usize,
    paths: usize,
    decoded: usize,
}
fn charge(used: &mut usize, amount: usize, limit: usize, name: &'static str) -> Result<(), Error> {
    let next = used
        .checked_add(amount)
        .filter(|n| *n <= limit)
        .ok_or(Error::Budget(name))?;
    *used = next;
    Ok(())
}
impl Budget {
    fn work(&mut self, n: usize) -> Result<(), Error> {
        charge(&mut self.work, n, MAX_WORK, "work")
    }
    fn storage(&mut self, n: usize) -> Result<(), Error> {
        charge(&mut self.storage, n, MAX_STORAGE_BYTES, "allocated storage")
    }
    fn reserve<T>(&mut self, v: &mut Vec<T>, additional: usize) -> Result<(), Error> {
        let needed = v
            .len()
            .checked_add(additional)
            .ok_or(Error::Budget("allocated storage"))?;
        if needed > v.capacity() {
            let before = v.capacity();
            let target = needed.max(before.saturating_mul(2)).max(8);
            let bytes = (target - before)
                .checked_mul(size_of::<T>())
                .ok_or(Error::Budget("allocated storage"))?;
            self.storage(bytes)?;
            v.try_reserve_exact(target - v.len())
                .map_err(|_| Error::Budget("allocation"))?;
            self.storage(
                (v.capacity() - target)
                    .checked_mul(size_of::<T>())
                    .ok_or(Error::Budget("allocated storage"))?,
            )?;
        }
        Ok(())
    }
    fn push<T>(&mut self, v: &mut Vec<T>, value: T) -> Result<(), Error> {
        self.reserve(v, 1)?;
        v.push(value);
        Ok(())
    }
    fn string(&mut self, value: &str) -> Result<String, Error> {
        let mut bytes = Vec::new();
        self.reserve(&mut bytes, value.len())?;
        bytes.extend_from_slice(value.as_bytes());
        // Input was UTF-8; this conversion does not copy or allocate.
        String::from_utf8(bytes).map_err(|_| Error::Budget("UTF-8 storage"))
    }
    fn path_copy(&mut self, path: &Path) -> Result<PathBuf, Error> {
        let text = path
            .to_str()
            .ok_or_else(|| invalid("", 0, "non-UTF-8 path"))?;
        charge(
            &mut self.paths,
            text.len(),
            MAX_TOTAL_PATH_BYTES,
            "aggregate paths",
        )?;
        Ok(PathBuf::from(self.string(text)?))
    }
    fn source(&mut self, path: &Path, range: Range<usize>) -> Result<Source, Error> {
        Ok(Source {
            document: self.path_copy(path)?,
            range,
        })
    }
}

fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\r' | '\n' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}
fn whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}
fn name(value: &str) -> bool {
    value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphabetic)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}
struct Element<'a> {
    name: &'a str,
    attrs: Vec<(&'a str, Cow<'a, str>)>,
    children: Vec<usize>,
    range: Range<usize>,
}
impl Element<'_> {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_ref())
    }
    fn required(&self, key: &str, doc: &str) -> Result<&str, Error> {
        self.attr(key)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| invalid(doc, self.range.start, "missing required attribute"))
    }
    fn schema(
        &self,
        attrs: &[&str],
        children: &[&str],
        nodes: &[Element<'_>],
        doc: &str,
        b: &mut Budget,
    ) -> Result<(), Error> {
        for (key, _) in &self.attrs {
            b.work(attrs.len())?;
            if !attrs.contains(key) {
                return Err(unsupported(
                    doc,
                    self.range.start,
                    "unknown attribute or namespace",
                ));
            }
        }
        for child in &self.children {
            b.work(children.len())?;
            if !children.contains(&nodes[*child].name) {
                return Err(unsupported(
                    doc,
                    nodes[*child].range.start,
                    "unknown or source-affecting element",
                ));
            }
        }
        Ok(())
    }
}
fn decode<'a>(
    raw: &'a [u8],
    doc: &str,
    offset: usize,
    b: &mut Budget,
) -> Result<Cow<'a, str>, Error> {
    let raw =
        std::str::from_utf8(raw).map_err(|_| invalid(doc, offset, "attribute is not UTF-8"))?;
    if raw.contains('<') {
        return Err(invalid(doc, offset, "literal < in attribute"));
    }
    charge(
        &mut b.decoded,
        raw.len(),
        MAX_TOTAL_BYTES,
        "decoded attribute bytes",
    )?;
    if !raw.contains(['&', '\t', '\r', '\n']) {
        return Ok(Cow::Borrowed(raw));
    }
    let mut out = Vec::new();
    b.reserve(&mut out, raw.len())?;
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        let c = if c == '&' {
            let start = chars.clone();
            let mut length = 0;
            let mut ended = false;
            for c in chars.by_ref() {
                if c == ';' {
                    ended = true;
                    break;
                }
                length += c.len_utf8();
            }
            if !ended {
                return Err(invalid(doc, offset, "unterminated entity"));
            }
            let remaining = start.as_str();
            let entity = &remaining[..length];
            match entity {
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "quot" => '"',
                "apos" => '\'',
                _ => {
                    let (digits, radix) = if let Some(s) = entity.strip_prefix("#x") {
                        (s, 16)
                    } else if let Some(s) = entity.strip_prefix('#') {
                        (s, 10)
                    } else {
                        return Err(unsupported(doc, offset, "non-predefined entity"));
                    };
                    if digits.is_empty()
                        || !digits.bytes().all(|c| {
                            if radix == 16 {
                                c.is_ascii_hexdigit()
                            } else {
                                c.is_ascii_digit()
                            }
                        })
                    {
                        return Err(invalid(doc, offset, "invalid character reference"));
                    }
                    u32::from_str_radix(digits, radix)
                        .ok()
                        .and_then(char::from_u32)
                        .filter(|c| xml_char(*c))
                        .ok_or_else(|| invalid(doc, offset, "illegal XML character reference"))?
                }
            }
        } else if c == '\r' {
            if chars.clone().next() == Some('\n') {
                chars.next();
            }
            ' '
        } else if matches!(c, '\t' | '\n') {
            ' '
        } else {
            c
        };
        let mut buf = [0; 4];
        out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
    }
    Ok(Cow::Owned(
        String::from_utf8(out).map_err(|_| invalid(doc, offset, "invalid UTF-8"))?,
    ))
}
fn attributes<'a>(
    raw: &'a str,
    name_len: usize,
    doc: &str,
    offset: usize,
    b: &mut Budget,
) -> Result<Vec<(&'a str, Cow<'a, str>)>, Error> {
    // Map validated borrowed attributes back into the original input lifetime.
    // Disable quick-xml's allocating duplicate checker; the bounded scan below replaces it.
    let borrowed = BytesStart::from_content(raw, name_len);
    let mut attrs = Vec::new();
    for attr in borrowed.attributes().with_checks(false) {
        if attrs.len() == MAX_ATTRIBUTES {
            return Err(Error::Budget("attributes per element"));
        }
        let attr = attr.map_err(|_| invalid(doc, offset, "malformed attribute"))?;
        b.work(attrs.len() + 1)?;
        let index = attr
            .key
            .as_ref()
            .as_ptr()
            .addr()
            .checked_sub(raw.as_ptr().addr())
            .ok_or_else(|| invalid(doc, offset, "attribute position"))?;
        let key = raw
            .get(index..index + attr.key.as_ref().len())
            .ok_or_else(|| invalid(doc, offset, "invalid attribute name"))?;
        if index == 0
            || !raw
                .as_bytes()
                .get(index - 1)
                .is_some_and(|c| matches!(c, b' ' | b'\t' | b'\r' | b'\n'))
        {
            return Err(invalid(doc, offset, "attributes require XML whitespace"));
        }
        if !name(key) {
            return Err(unsupported(doc, offset, "attribute namespace or name"));
        }
        if attrs.iter().any(|(k, _)| *k == key) {
            return Err(invalid(doc, offset, "duplicate attribute"));
        }
        let raw_value = match attr.value {
            Cow::Borrowed(v) => v,
            Cow::Owned(_) => return Err(invalid(doc, offset, "expected borrowed attribute")),
        };
        let value_index = raw_value
            .as_ptr()
            .addr()
            .checked_sub(raw.as_ptr().addr())
            .ok_or_else(|| invalid(doc, offset, "attribute position"))?;
        let value = decode(
            raw.as_bytes()
                .get(value_index..value_index + raw_value.len())
                .ok_or_else(|| invalid(doc, offset, "attribute position"))?,
            doc,
            offset,
            b,
        )?;
        b.push(&mut attrs, (key, value))?;
    }
    Ok(attrs)
}
fn parse<'a>(doc: &str, bytes: &'a [u8], b: &mut Budget) -> Result<Vec<Element<'a>>, Error> {
    let text = std::str::from_utf8(bytes).map_err(|_| invalid(doc, 0, "document is not UTF-8"))?;
    if !text.chars().all(xml_char) {
        return Err(invalid(doc, 0, "illegal XML scalar"));
    }
    // quick-xml's opening-name bytes grow geometrically. Include minimum capacity
    // and its index stack (the reader sees depth33 before our depth32 refusal).
    let scratch = bytes
        .len()
        .checked_mul(2)
        .and_then(|n| n.checked_add(2 * MAX_DEPTH * size_of::<usize>() + 8))
        .ok_or(Error::Budget("allocated storage"))?;
    b.storage(scratch)?;
    let bom = usize::from(text.starts_with('\u{feff}')) * 3;
    let mut reader = Reader::from_str(text);
    reader.config_mut().check_comments = true;
    let mut nodes: Vec<Element<'a>> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    loop {
        charge(&mut b.events, 1, MAX_EVENTS, "aggregate XML events")?;
        let start = reader.buffer_position() as usize + bom;
        let event = reader
            .read_event()
            .map_err(|_| invalid(doc, start, "malformed XML"))?;
        let end = reader.buffer_position() as usize + bom;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(tag) | Event::Empty(tag) => {
                if stack.is_empty() && !nodes.is_empty() {
                    return Err(invalid(doc, start, "multiple roots"));
                }
                if stack.len() == MAX_DEPTH {
                    return Err(Error::Budget("XML depth"));
                }
                let raw_start = end - tag.as_ref().len() - if empty { 2 } else { 1 };
                let raw = text
                    .get(raw_start..raw_start + tag.as_ref().len())
                    .ok_or_else(|| invalid(doc, start, "tag position"))?;
                let tag_name = &raw[..tag.name().as_ref().len()];
                if !name(tag_name) {
                    return Err(unsupported(doc, start, "element namespace or name"));
                }
                let attrs = attributes(raw, tag_name.len(), doc, start, b)?;
                let index = nodes.len();
                if let Some(parent) = stack.last().copied() {
                    b.push(&mut nodes[parent].children, index)?;
                }
                b.push(
                    &mut nodes,
                    Element {
                        name: tag_name,
                        attrs,
                        children: Vec::new(),
                        range: raw_start - 1..end,
                    },
                )?;
                if !empty {
                    b.push(&mut stack, index)?;
                }
            }
            Event::End(_) => {
                let index = stack
                    .pop()
                    .ok_or_else(|| invalid(doc, start, "unmatched end"))?;
                nodes[index].range.end = end;
            }
            Event::Text(value) => {
                let value = value
                    .decode()
                    .map_err(|_| invalid(doc, start, "invalid text"))?;
                if !value.chars().all(whitespace) {
                    return Err(invalid(doc, start, "only whitespace text is allowed"));
                }
            }
            Event::Comment(_) => {}
            Event::Decl(_) => {
                if start != bom {
                    return Err(invalid(doc, start, "declaration must be first"));
                }
                let raw = text
                    .get(start + 2..end - 2)
                    .ok_or_else(|| invalid(doc, start, "invalid declaration"))?;
                if raw.contains('&') {
                    return Err(invalid(doc, start, "references in XML declaration"));
                }
                let attrs = attributes(raw, 3, doc, start, b)?;
                if attrs.is_empty()
                    || attrs[0].0 != "version"
                    || attrs[0].1 != "1.0"
                    || attrs.len() > 3
                {
                    return Err(unsupported(doc, start, "expected XML 1.0 declaration"));
                }
                let mut standalone = false;
                for (key, value) in attrs.iter().skip(1) {
                    match (*key, value.as_ref()) {
                        ("encoding", "UTF-8" | "utf-8") if !standalone => {}
                        ("standalone", "yes" | "no") => standalone = true,
                        _ => return Err(unsupported(doc, start, "declaration encoding or order")),
                    }
                }
            }
            Event::Eof => {
                if !stack.is_empty() || nodes.is_empty() {
                    return Err(invalid(doc, start, "missing or unclosed root"));
                }
                return Ok(nodes);
            }
            _ => {
                return Err(unsupported(
                    doc,
                    start,
                    "DTD, entities, CDATA, XInclude or processing instructions",
                ));
            }
        }
    }
}

fn lexical(
    value: &str,
    relative: Option<&Path>,
    doc: &str,
    offset: usize,
    b: &mut Budget,
) -> Result<PathBuf, Error> {
    // Bound caller snapshot keys before any diagnostic can retain their spelling.
    if value.len() > MAX_PATH_BYTES {
        return Err(Error::Budget("path bytes"));
    }
    if value.is_empty()
        || value.chars().any(|c| {
            c.is_control()
                || matches!(c, '$' | '<' | '>' | '*' | '?' | '|' | '"')
                || (c == '\\' && !cfg!(windows))
        })
        || value.starts_with("//")
        || value.starts_with("\\\\")
        || value.contains("://")
    {
        return Err(unsupported(
            doc,
            offset,
            "unsupported path or unresolved macro",
        ));
    }
    let path = Path::new(value);
    #[cfg(windows)]
    if matches!(path.components().next(), Some(Component::Prefix(p)) if !matches!(p.kind(), std::path::Prefix::Disk(_)))
        || (path.has_root() && !path.is_absolute())
        || (matches!(path.components().next(), Some(Component::Prefix(_))) && !path.is_absolute())
    {
        return Err(unsupported(
            doc,
            offset,
            "UNC, device or drive-relative path",
        ));
    }
    // Colons are only permitted in a native Windows drive prefix, never guessed on Unix.
    if path.components().any(|component| {
        !matches!(component, Component::Prefix(_))
            && component.as_os_str().as_encoded_bytes().contains(&b':')
    }) {
        return Err(unsupported(doc, offset, "foreign path or scheme"));
    }
    let prefix = if path.is_absolute() {
        None
    } else {
        Some(relative.ok_or_else(|| invalid(doc, offset, "expected absolute path"))?)
    };
    let length = value
        .len()
        .checked_add(prefix.map_or(0, |p| p.as_os_str().len() + 1))
        .ok_or(Error::Budget("path bytes"))?;
    if length > MAX_PATH_BYTES {
        return Err(Error::Budget("path bytes"));
    }
    charge(
        &mut b.paths,
        length,
        MAX_TOTAL_PATH_BYTES,
        "aggregate paths",
    )?;
    // Reserve the entire pre-normalization spelling, including the separator.
    b.storage(length)?;
    let mut output = PathBuf::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| Error::Budget("allocation"))?;
    b.storage(output.capacity().saturating_sub(length))?;
    if let Some(prefix) = prefix {
        output.push(prefix);
    }
    for component in path.components() {
        b.work(1)?;
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !output.pop() {
                    return Err(invalid(doc, offset, "path escapes filesystem root"));
                }
            }
            other => output.push(other.as_os_str()),
        }
    }
    if !output.is_absolute() {
        return Err(invalid(doc, offset, "expected absolute host path"));
    }
    Ok(output)
}
fn resolve(
    value: &str,
    url: bool,
    project: &Path,
    module: Option<&Path>,
    doc: &str,
    offset: usize,
    b: &mut Budget,
) -> Result<PathBuf, Error> {
    if value.len() > MAX_PATH_BYTES {
        return Err(Error::Budget("path bytes"));
    }
    let value = if url {
        value
            .strip_prefix("file://")
            .ok_or_else(|| unsupported(doc, offset, "only file URLs are supported"))?
    } else {
        value
    };
    if url && value.contains(['?', '#']) {
        return Err(unsupported(doc, offset, "URL query or fragment"));
    }
    let mut decoded = Vec::new();
    let value = if url && value.contains('%') {
        b.reserve(&mut decoded, value.len())?;
        let mut index = 0;
        while index < value.len() {
            let c = value.as_bytes()[index];
            if c == b'%' {
                let digits = value
                    .get(index + 1..index + 3)
                    .ok_or_else(|| invalid(doc, offset, "invalid URL escape"))?;
                let c = u8::from_str_radix(digits, 16)
                    .map_err(|_| invalid(doc, offset, "invalid URL escape"))?;
                if matches!(c, 0 | b'/' | b'\\' | b'$') {
                    return Err(unsupported(doc, offset, "encoded separator, macro or NUL"));
                }
                decoded.push(c);
                index += 3;
            } else {
                decoded.push(c);
                index += 1;
            }
        }
        std::str::from_utf8(&decoded).map_err(|_| invalid(doc, offset, "URL is not UTF-8"))?
    } else {
        value
    };
    let (base, suffix) = if let Some(rest) = value.strip_prefix("$PROJECT_DIR$") {
        (Some(project), rest)
    } else if let Some(rest) = value.strip_prefix("$MODULE_DIR$") {
        (
            Some(module.ok_or_else(|| unsupported(doc, offset, "MODULE_DIR outside module"))?),
            rest,
        )
    } else {
        (None, value)
    };
    if let Some(base) = base {
        if !suffix.is_empty() && !suffix.starts_with('/') {
            return Err(invalid(doc, offset, "macro requires path boundary"));
        }
        if suffix.starts_with("//") {
            return Err(invalid(doc, offset, "double separator after macro"));
        }
        if suffix.is_empty() || suffix == "/" {
            return b.path_copy(base);
        }
        let relative = Path::new(&suffix[1..]);
        if relative.has_root() || matches!(relative.components().next(), Some(Component::Prefix(_)))
        {
            return Err(unsupported(
                doc,
                offset,
                "macro suffix must remain relative",
            ));
        }
        return lexical(&suffix[1..], Some(base), doc, offset, b);
    }
    #[cfg(windows)]
    let value = if url && value.starts_with('/') && value.as_bytes().get(2) == Some(&b':') {
        &value[1..]
    } else {
        value
    };
    lexical(value, None, doc, offset, b)
}
fn boolean(element: &Element<'_>, key: &str, doc: &str) -> Result<bool, Error> {
    match element.attr(key) {
        None | Some("false") => Ok(false),
        Some("true") => Ok(true),
        _ => Err(invalid(doc, element.range.start, "expected true or false")),
    }
}
fn only_child<'a>(
    node: &Element<'_>,
    nodes: &'a [Element<'a>],
    doc: &str,
) -> Result<&'a Element<'a>, Error> {
    if node.children.len() != 1 {
        return Err(invalid(doc, node.range.start, "expected exactly one child"));
    }
    Ok(&nodes[node.children[0]])
}
struct Input<'a> {
    path: PathBuf,
    bytes: &'a [u8],
    used: bool,
}
struct Reference {
    path: PathBuf,
    range: Range<usize>,
}

/// Atomically project a complete caller snapshot. Supplied but unreferenced documents refuse.
/// Outputs/language/order/SDK semantics are recognized but explicitly outside this operation;
/// exclusions, resources, package prefixes, custom roots, facets and unknown XML refuse.
pub fn source_projection(
    project_directory: impl AsRef<Path>,
    documents: &[Document<'_>],
) -> Result<ImportedSources, Error> {
    if documents.len() > MAX_DOCUMENTS {
        return Err(Error::Budget("documents"));
    }
    let mut b = Budget::default();
    // One bounded diagnostic document string can outlive a failed operation.
    b.storage(MAX_PATH_BYTES)?;
    let base = project_directory
        .as_ref()
        .to_str()
        .ok_or_else(|| invalid("", 0, "base is not UTF-8"))?;
    let project = lexical(base, None, "", 0, &mut b)?;
    let manifest = lexical(".idea/modules.xml", Some(&project), "", 0, &mut b)?;
    let mut inputs: Vec<Input<'_>> = Vec::new();
    for document in documents {
        if document.bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(Error::Budget("document bytes"));
        }
        charge(
            &mut b.bytes,
            document.bytes.len(),
            MAX_TOTAL_BYTES,
            "aggregate document bytes",
        )?;
        let path = lexical(document.path, Some(&project), document.path, 0, &mut b)?;
        b.work(inputs.len())?;
        if inputs.iter().any(|i| i.path == path) {
            return Err(invalid(
                document.path,
                0,
                "duplicate normalized snapshot key",
            ));
        }
        b.push(
            &mut inputs,
            Input {
                path,
                bytes: document.bytes,
                used: false,
            },
        )?;
    }
    b.work(inputs.len())?;
    let index = inputs
        .iter()
        .position(|i| i.path == manifest)
        .ok_or_else(|| invalid(".idea/modules.xml", 0, "missing manifest"))?;
    inputs[index].used = true;
    let nodes = parse(".idea/modules.xml", inputs[index].bytes, &mut b)?;
    let root = &nodes[0];
    root.schema(
        &["version"],
        &["component"],
        &nodes,
        ".idea/modules.xml",
        &mut b,
    )?;
    if root.name != "project" || root.attr("version") != Some("4") {
        return Err(unsupported(
            ".idea/modules.xml",
            0,
            "expected project version 4",
        ));
    }
    let component = only_child(root, &nodes, ".idea/modules.xml")?;
    component.schema(&["name"], &["modules"], &nodes, ".idea/modules.xml", &mut b)?;
    if component.attr("name") != Some("ProjectModuleManager") {
        return Err(unsupported(
            ".idea/modules.xml",
            component.range.start,
            "expected ProjectModuleManager",
        ));
    }
    let list = only_child(component, &nodes, ".idea/modules.xml")?;
    list.schema(&[], &["module"], &nodes, ".idea/modules.xml", &mut b)?;
    let mut references: Vec<Reference> = Vec::new();
    for child in &list.children {
        let node = &nodes[*child];
        node.schema(
            &["filepath", "fileurl", "group"],
            &[],
            &nodes,
            ".idea/modules.xml",
            &mut b,
        )?;
        let path = resolve(
            node.required("filepath", ".idea/modules.xml")?,
            false,
            &project,
            None,
            ".idea/modules.xml",
            node.range.start,
            &mut b,
        )?;
        if let Some(url) = node.attr("fileurl") {
            let other = resolve(
                url,
                true,
                &project,
                None,
                ".idea/modules.xml",
                node.range.start,
                &mut b,
            )?;
            if path != other {
                return Err(invalid(
                    ".idea/modules.xml",
                    node.range.start,
                    "fileurl and filepath disagree",
                ));
            }
        }
        if path.extension().is_none_or(|s| s != "iml") {
            return Err(invalid(
                ".idea/modules.xml",
                node.range.start,
                "module reference must end in .iml",
            ));
        }
        b.work(references.len())?;
        if references.iter().any(|r| r.path == path) {
            return Err(invalid(
                ".idea/modules.xml",
                node.range.start,
                "duplicate module reference",
            ));
        }
        if references.len() >= MAX_DOCUMENTS - 1 {
            return Err(Error::Budget("module references"));
        }
        b.push(
            &mut references,
            Reference {
                path,
                range: node.range.clone(),
            },
        )?;
    }
    let mut modules: Vec<Module> = Vec::new();
    for reference in references {
        let doc = reference
            .path
            .to_str()
            .ok_or_else(|| invalid("", 0, "non-UTF-8 document"))?;
        let id = reference
            .path
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| invalid(doc, 0, "empty module filename identity"))?;
        b.work(modules.len() + inputs.len())?;
        if modules.iter().any(|m| m.id == id) {
            return Err(invalid(doc, 0, "ambiguous module filename identity"));
        }
        let input = inputs
            .iter_mut()
            .find(|i| i.path == reference.path)
            .ok_or_else(|| invalid(doc, 0, "missing referenced module document"))?;
        input.used = true;
        let nodes = parse(doc, input.bytes, &mut b)?;
        let root = &nodes[0];
        root.schema(&["type", "version"], &["component"], &nodes, doc, &mut b)?;
        if root.name != "module"
            || root.attr("type") != Some("JAVA_MODULE")
            || root.attr("version") != Some("4")
        {
            return Err(unsupported(
                doc,
                root.range.start,
                "expected JAVA_MODULE version 4",
            ));
        }
        let manager = only_child(root, &nodes, doc)?;
        manager.schema(
            &["name", "LANGUAGE_LEVEL", "inherit-compiler-output"],
            &[
                "content",
                "output",
                "output-test",
                "exclude-output",
                "orderEntry",
            ],
            &nodes,
            doc,
            &mut b,
        )?;
        if manager.attr("name") != Some("NewModuleRootManager") {
            return Err(unsupported(
                doc,
                manager.range.start,
                "facets and unknown components",
            ));
        }
        boolean(manager, "inherit-compiler-output", doc)?;
        let directory = reference
            .path
            .parent()
            .ok_or_else(|| invalid(doc, 0, "module has no directory"))?;
        let mut roots = Vec::new();
        let mut singleton = [false; 3];
        let mut outputs = Vec::new();
        for child in &manager.children {
            let node = &nodes[*child];
            match node.name {
                "content" => {
                    node.schema(&["url"], &["sourceFolder"], &nodes, doc, &mut b)?;
                    let content = resolve(
                        node.required("url", doc)?,
                        true,
                        &project,
                        Some(directory),
                        doc,
                        node.range.start,
                        &mut b,
                    )?;
                    for child in &node.children {
                        let source = &nodes[*child];
                        source.schema(
                            &["url", "type", "isTestSource", "generated", "packagePrefix"],
                            &[],
                            &nodes,
                            doc,
                            &mut b,
                        )?;
                        if source.attr("packagePrefix").is_some_and(|p| !p.is_empty()) {
                            return Err(unsupported(
                                doc,
                                source.range.start,
                                "nonempty package prefix",
                            ));
                        }
                        let test = boolean(source, "isTestSource", doc)?;
                        let generated = boolean(source, "generated", doc)?;
                        let kind = match source.attr("type") {
                            Some("java-source") => SourceRootKind::Production,
                            Some("java-test") => SourceRootKind::Test,
                            None if test => SourceRootKind::Test,
                            None => SourceRootKind::Production,
                            _ => {
                                return Err(unsupported(
                                    doc,
                                    source.range.start,
                                    "resource or custom source root type",
                                ));
                            }
                        };
                        let path = resolve(
                            source.required("url", doc)?,
                            true,
                            &project,
                            Some(directory),
                            doc,
                            source.range.start,
                            &mut b,
                        )?;
                        if !path.starts_with(&content) {
                            return Err(unsupported(
                                doc,
                                source.range.start,
                                "source root outside containing content",
                            ));
                        }
                        if roots.len() >= ktlsp_project::MAX_SOURCE_ROOTS {
                            return Err(Error::Budget("source roots"));
                        }
                        let provenance = b.source(&reference.path, source.range.clone())?;
                        b.push(
                            &mut roots,
                            SourceRoot {
                                path,
                                kind,
                                generated,
                                source: provenance,
                            },
                        )?;
                    }
                }
                "output" | "output-test" | "exclude-output" => {
                    let slot = match node.name {
                        "output" => 0,
                        "output-test" => 1,
                        _ => 2,
                    };
                    if singleton[slot] {
                        return Err(invalid(doc, node.range.start, "duplicate output setting"));
                    }
                    singleton[slot] = true;
                    if slot == 2 {
                        node.schema(&[], &[], &nodes, doc, &mut b)?;
                    } else {
                        node.schema(&["url"], &[], &nodes, doc, &mut b)?;
                        let path = resolve(
                            node.required("url", doc)?,
                            true,
                            &project,
                            Some(directory),
                            doc,
                            node.range.start,
                            &mut b,
                        )?;
                        b.push(&mut outputs, path)?;
                    }
                }
                "orderEntry" => {
                    let allowed: &[&str] = match node.required("type", doc)? {
                        "sourceFolder" => &["type", "forTests"],
                        "inheritedJdk" => &["type"],
                        "jdk" => &["type", "jdkName", "jdkType"],
                        "module" => &[
                            "type",
                            "module-name",
                            "scope",
                            "exported",
                            "production-on-test",
                        ],
                        "library" => &["type", "name", "level", "scope", "exported"],
                        _ => {
                            return Err(unsupported(
                                doc,
                                node.range.start,
                                "unsupported order entry schema",
                            ));
                        }
                    };
                    node.schema(allowed, &[], &nodes, doc, &mut b)?;
                    // These original dependency/SDK/order attributes are deliberately not a classpath.
                }
                _ => {
                    return Err(unsupported(
                        doc,
                        node.range.start,
                        "unknown source-affecting XML",
                    ));
                }
            }
        }
        if singleton[2] {
            b.work(roots.len() * outputs.len())?;
            if roots.iter().any(|root| {
                outputs
                    .iter()
                    .any(|out| root.path.starts_with(out) || out.starts_with(&root.path))
            }) {
                return Err(unsupported(
                    doc,
                    manager.range.start,
                    "declared source intersects excluded compiler output",
                ));
            }
        }
        let module = Module {
            id: b.string(id)?,
            source_roots: roots,
            source: b.source(&reference.path, root.range.clone())?,
            declaration: b.source(&manifest, reference.range)?,
        };
        b.push(&mut modules, module)?;
    }
    if inputs.iter().any(|i| !i.used) {
        return Err(invalid("", 0, "unreferenced snapshot document"));
    }
    #[derive(Serialize)]
    struct Projection<'a> {
        #[serde(rename = "schemaVersion")]
        version: u8,
        modules: &'a [Module],
    }
    let mut output = ProjectionWriter {
        bytes: Vec::new(),
        budget: &mut b,
        failure: None,
    };
    if let Err(error) = serde_json::to_writer(
        &mut output,
        &Projection {
            version: 1,
            modules: &modules,
        },
    ) {
        return Err(output.failure.take().unwrap_or(Error::Serialization(error)));
    }
    // The existing project loader owns bounded JSON/model validation allocations. Reserve a
    // conservative aggregate envelope BEFORE calling it: escaped bytes cover string copies,
    // and per-record space covers vector/hash capacities and overlap sorting scratch.
    let roots: usize = modules.iter().map(|m| m.source_roots.len()).sum();
    output.budget.work(
        (roots + modules.len())
            .checked_mul(32)
            .ok_or(Error::Budget("work"))?,
    )?;
    let envelope = output
        .bytes
        .len()
        .checked_add(manifest.as_os_str().len())
        .and_then(|n| n.checked_mul(8))
        .and_then(|n| n.checked_add((modules.len() + 1) * 2048))
        .and_then(|n| n.checked_add(roots * 512))
        .ok_or(Error::Budget("allocated storage"))?;
    output.budget.storage(envelope)?;
    let project = ktlsp_project::from_json(&manifest, &output.bytes).map_err(Error::Project)?;
    Ok(ImportedSources { project, modules })
}
struct ProjectionWriter<'a> {
    bytes: Vec<u8>,
    budget: &'a mut Budget,
    failure: Option<Error>,
}
impl io::Write for ProjectionWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = (|| {
            if self
                .bytes
                .len()
                .checked_add(bytes.len())
                .is_none_or(|n| n > ktlsp_project::MAX_MANIFEST_BYTES)
            {
                return Err(Error::Budget("projection bytes"));
            }
            self.budget.reserve(&mut self.bytes, bytes.len())?;
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        })();
        result.map_err(|e| {
            self.failure = Some(e);
            io::Error::other("JPS projection budget")
        })
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod boundaries {
    use super::*;
    use std::io::Write;

    #[test]
    fn counters_accept_exact_and_refuse_next_without_mutation_or_overflow() {
        for (limit, label) in [
            (MAX_TOTAL_BYTES, "bytes"),
            (MAX_TOTAL_BYTES, "decoded"),
            (MAX_EVENTS, "events"),
            (MAX_WORK, "work"),
            (MAX_STORAGE_BYTES, "storage"),
            (MAX_TOTAL_PATH_BYTES, "paths"),
        ] {
            let mut used = 0;
            charge(&mut used, limit, limit, label).unwrap();
            assert!(
                matches!(charge(&mut used, 1, limit, label), Err(Error::Budget(s)) if s == label)
            );
            assert_eq!(used, limit);
            assert!(charge(&mut used, usize::MAX, limit, label).is_err());
            assert_eq!(used, limit);
        }
    }

    #[test]
    fn xml_depth_and_attribute_boundaries_are_checked_before_retaining() {
        let exact = format!("{}{}", "<x>".repeat(MAX_DEPTH), "</x>".repeat(MAX_DEPTH));
        let nodes = parse("test", exact.as_bytes(), &mut Budget::default()).unwrap();
        assert_eq!(nodes.len(), MAX_DEPTH);
        let excessive = format!("<x>{exact}</x>");
        assert!(matches!(
            parse("test", excessive.as_bytes(), &mut Budget::default()),
            Err(Error::Budget("XML depth"))
        ));
        let attrs = (0..MAX_ATTRIBUTES)
            .map(|i| format!(" a{i}='x'"))
            .collect::<String>();
        let exact = format!("<x{attrs}/>");
        assert_eq!(
            parse("test", exact.as_bytes(), &mut Budget::default()).unwrap()[0]
                .attrs
                .len(),
            MAX_ATTRIBUTES
        );
        let excessive = format!("<x{attrs} extra='x'/>");
        assert!(matches!(
            parse("test", excessive.as_bytes(), &mut Budget::default()),
            Err(Error::Budget("attributes per element"))
        ));
    }

    #[test]
    fn decoded_bytes_storage_and_work_share_one_operation() {
        let mut budget = Budget {
            decoded: MAX_TOTAL_BYTES - 1,
            ..Budget::default()
        };
        assert_eq!(decode(b"x", "test", 0, &mut budget).unwrap(), "x");
        assert!(matches!(
            decode(b"x", "test", 0, &mut budget),
            Err(Error::Budget("decoded attribute bytes"))
        ));
        let mut budget = Budget {
            storage: MAX_STORAGE_BYTES,
            ..Budget::default()
        };
        assert!(matches!(
            budget.string("x"),
            Err(Error::Budget("allocated storage"))
        ));
        let mut budget = Budget {
            work: MAX_WORK,
            ..Budget::default()
        };
        assert!(matches!(budget.work(1), Err(Error::Budget("work"))));
        let mut budget = Budget::default();
        parse("one", b"<x a='&amp;'/>", &mut budget).unwrap();
        let previous = (budget.events, budget.storage, budget.decoded);
        parse("two", b"<x a='&amp;'/>", &mut budget).unwrap();
        assert!(
            budget.events > previous.0
                && budget.storage > previous.1
                && budget.decoded > previous.2
        );
    }

    #[test]
    fn path_prejoin_and_provenance_share_exact_limits() {
        let prefix = if cfg!(windows) { "C:/" } else { "/" };
        let value = format!("{prefix}{}", "x".repeat(MAX_PATH_BYTES - prefix.len()));
        let path = lexical(&value, None, "test", 0, &mut Budget::default()).unwrap();
        assert_eq!(path.as_os_str().len(), MAX_PATH_BYTES);
        assert!(matches!(
            lexical(&(value + "x"), None, "test", 0, &mut Budget::default()),
            Err(Error::Budget("path bytes"))
        ));
        let path = Path::new(if cfg!(windows) { "C:/x" } else { "/x" });
        let length = path.as_os_str().len();
        let mut budget = Budget {
            paths: MAX_TOTAL_PATH_BYTES - length,
            ..Budget::default()
        };
        budget.source(path, 0..1).unwrap();
        assert!(matches!(
            budget.source(path, 0..1),
            Err(Error::Budget("aggregate paths"))
        ));
        assert_eq!(budget.paths, MAX_TOTAL_PATH_BYTES);
    }

    #[test]
    fn serialized_output_limit_refuses_before_retention() {
        let mut budget = Budget::default();
        let mut writer = ProjectionWriter {
            bytes: Vec::new(),
            budget: &mut budget,
            failure: None,
        };
        writer
            .write_all(&vec![b' '; ktlsp_project::MAX_MANIFEST_BYTES])
            .unwrap();
        assert_eq!(writer.bytes.len(), ktlsp_project::MAX_MANIFEST_BYTES);
        assert!(writer.write_all(b" ").is_err());
        assert!(matches!(
            writer.failure,
            Some(Error::Budget("projection bytes"))
        ));
        assert_eq!(writer.bytes.len(), ktlsp_project::MAX_MANIFEST_BYTES);
    }

    #[test]
    fn xml_reader_minimum_stack_storage_is_reserved_before_reading() {
        let mut budget = Budget {
            storage: MAX_STORAGE_BYTES - 6,
            ..Budget::default()
        };
        // The input-byte-only allowance cannot cover even the reader's minimum
        // opening-name and index capacities; refuse before the unclosed tag.
        assert!(matches!(
            parse("tiny", b"<x>", &mut budget),
            Err(Error::Budget("allocated storage"))
        ));
        assert_eq!(budget.storage, MAX_STORAGE_BYTES - 6);
    }
}
