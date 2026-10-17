//! Literal, data-only POM snapshots, not Maven effective models or resolution.
//! See `reviews/t022-static-maven.md` for the strict subset and omitted defaults.

use ktlsp_project::{DependencyScope, ProjectModel, SourceRootKind};
use quick_xml::{
    NsReader,
    events::{BytesStart, Event},
};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    fmt, io,
    ops::Range,
    path::{Component, Path, PathBuf},
};

pub const MAX_POMS: usize = 128;
pub const MAX_POM_BYTES: usize = 64 * 1024;
pub const MAX_TOTAL_BYTES: usize = 1024 * 1024;
pub const MAX_EVENTS: usize = 8192;
pub const MAX_DEPTH: usize = 32;
pub const MAX_PATH_BYTES: usize = 4096;
pub const MAX_TOTAL_PATH_BYTES: usize = 1024 * 1024;
const NS: &[u8] = b"http://maven.apache.org/POM/4.0.0";
const XSI: &[u8] = b"http://www.w3.org/2001/XMLSchema-instance";

/// Root is `pom.xml`; other keys must be explicitly reachable reactor POMs.
pub struct Pom<'a> {
    pub path: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Debug)]
pub enum Error {
    Budget(&'static str),
    Invalid {
        pom: String,
        offset: usize,
        reason: &'static str,
    },
    Unsupported {
        pom: String,
        offset: usize,
        reason: &'static str,
    },
    Project(ktlsp_project::ProjectError),
    Projection(serde_json::Error),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Budget(name) => write!(f, "Maven {name} budget exceeded"),
            Self::Invalid {
                pom,
                offset,
                reason,
            } => write!(f, "invalid POM {pom} at {offset}: {reason}"),
            Self::Unsupported {
                pom,
                offset,
                reason,
            } => write!(f, "unsupported POM {pom} at {offset}: {reason}"),
            Self::Project(error) => write!(f, "invalid Maven source projection: {error}"),
            Self::Projection(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Project(error) => Some(error),
            Self::Projection(error) => Some(error),
            _ => None,
        }
    }
}
fn invalid(pom: &str, offset: usize, reason: &'static str) -> Error {
    Error::Invalid {
        pom: pom.into(),
        offset,
        reason,
    }
}
fn unsupported(pom: &str, offset: usize, reason: &'static str) -> Error {
    Error::Unsupported {
        pom: pom.into(),
        offset,
        reason,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Coordinate {
    pub group: String,
    pub artifact: String,
    pub version: String,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Source {
    pub pom: PathBuf,
    /// Full element bytes in the caller snapshot; None denotes a documented default.
    pub range: Option<Range<usize>>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct SourceRoot {
    pub path: PathBuf,
    pub kind: SourceRootKind,
    pub source: Source,
}
#[derive(Debug, PartialEq, Eq)]
pub struct UnresolvedDependency {
    pub coordinate: Coordinate,
    pub scope: DependencyScope,
    pub scope_defaulted: bool,
    pub source: Source,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Module {
    pub coordinate: Coordinate,
    pub packaging: String,
    pub source: Source,
    pub source_roots: Vec<SourceRoot>,
    pub dependencies: Vec<UnresolvedDependency>,
}
/// Immutable declared reactor/source records; dependency occurrences remain unresolved.
/// ```compile_fail
/// fn mutate(mut w: ktlsp_import::maven::ImportedMaven) { w.modules.clear(); }
/// ```
#[derive(Debug)]
pub struct ImportedMaven {
    project: ProjectModel,
    /// Root-first declared reactor preorder. All dependency occurrences stay unresolved.
    modules: Vec<Module>,
}
impl ImportedMaven {
    /// Original root-first reactor preorder with source and dependency provenance.
    pub fn modules(&self) -> &[Module] {
        &self.modules
    }
    /// Source roots only: no dependency edges, libraries, SDK or effective build settings.
    pub fn source_projection(&self) -> &ProjectModel {
        &self.project
    }
}

struct Node {
    name: String,
    text: String,
    children: Vec<Node>,
    range: Range<usize>,
}
fn whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}
fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\r' | '\n' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}
fn attribute_separator(
    tag: &BytesStart<'_>,
    key: &[u8],
    pom: &str,
    offset: usize,
) -> Result<(), Error> {
    // quick-xml permits adjacent quoted attributes; XML requires separating whitespace.
    let separator = key
        .as_ptr()
        .addr()
        .checked_sub(tag.as_ref().as_ptr().addr())
        .and_then(|index| index.checked_sub(1))
        .and_then(|index| tag.as_ref().get(index));
    if !separator.is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n')) {
        return Err(invalid(pom, offset, "attributes require XML whitespace"));
    }
    Ok(())
}
fn attributes(tag: &BytesStart<'_>, root: bool, pom: &str, offset: usize) -> Result<(), Error> {
    let mut xsi = false;
    let mut schema = false;
    let mut namespace = false;
    for attribute in tag.attributes() {
        let attribute =
            attribute.map_err(|_| invalid(pom, offset, "invalid or duplicate attribute"))?;
        attribute_separator(tag, attribute.key.as_ref(), pom, offset)?;
        if !root {
            return Err(unsupported(
                pom,
                offset,
                "attributes or namespace redeclarations",
            ));
        }
        if attribute.value.iter().any(|b| matches!(b, b'&' | b'<')) {
            return Err(unsupported(
                pom,
                offset,
                "references in metadata attributes",
            ));
        }
        match attribute.key.as_ref() {
            b"xmlns" if attribute.value.as_ref() == NS => namespace = true,
            b"xmlns:xsi" if attribute.value.as_ref() == XSI => xsi = true,
            b"xsi:schemaLocation" if attribute.value.as_ref() == b"http://maven.apache.org/POM/4.0.0 https://maven.apache.org/xsd/maven-4.0.0.xsd" => schema = true,
            _ => return Err(unsupported(pom, offset, "attribute or namespace outside literal subset")),
        }
    }
    if schema && !(xsi && namespace) {
        return Err(invalid(
            pom,
            offset,
            "schemaLocation requires Maven and xsi namespaces",
        ));
    }
    Ok(())
}
fn parse(pom: &str, text: &str) -> Result<Node, Error> {
    if !text.chars().all(xml_char) {
        return Err(invalid(pom, 0, "invalid XML 1.0 character"));
    }
    let mut reader = NsReader::from_str(text);
    // quick-xml strips the UTF-8 BOM without charging it to buffer_position.
    let bom_bytes = usize::from(text.starts_with('\u{feff}')) * 3;
    reader.config_mut().check_comments = true;
    let mut stack: Vec<Node> = Vec::new();
    let mut root = None;
    for count in 0.. {
        if count == MAX_EVENTS {
            return Err(Error::Budget("XML events"));
        }
        let start = reader.buffer_position() as usize + bom_bytes;
        let event = reader
            .read_event()
            .map_err(|_| invalid(pom, start, "malformed XML"))?;
        let end = reader.buffer_position() as usize + bom_bytes;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(tag) | Event::Empty(tag) => {
                if stack.is_empty() && root.is_some() {
                    return Err(invalid(pom, start, "multiple roots"));
                }
                if stack.len() == MAX_DEPTH {
                    return Err(Error::Budget("XML depth"));
                }
                let raw_name = tag.name();
                let name = std::str::from_utf8(raw_name.as_ref())
                    .map_err(|_| invalid(pom, start, "invalid element name"))?;
                if !name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
                    || !name.bytes().all(|b| b.is_ascii_alphanumeric())
                {
                    return Err(unsupported(
                        pom,
                        start,
                        "element name or prefix outside subset",
                    ));
                }
                attributes(&tag, stack.is_empty(), pom, start)?;
                let start = end - tag.as_ref().len() - if empty { 3 } else { 2 };
                let node = Node {
                    name: name.into(),
                    text: String::new(),
                    children: Vec::new(),
                    range: start..end,
                };
                if empty {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(node);
                    } else {
                        root = Some(node);
                    }
                } else {
                    stack.push(node);
                }
            }
            Event::End(_) => {
                let mut node = stack
                    .pop()
                    .ok_or_else(|| invalid(pom, start, "unmatched end"))?;
                node.range.end = end;
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    root = Some(node);
                }
            }
            Event::Text(value) => {
                let value = value
                    .decode()
                    .map_err(|_| invalid(pom, start, "invalid text"))?;
                if value.contains("]]>") {
                    return Err(invalid(pom, start, "CDATA terminator in text"));
                }
                if let Some(parent) = stack.last_mut() {
                    parent.text.push_str(&value);
                } else if !value.chars().all(whitespace) {
                    return Err(invalid(pom, start, "text outside root"));
                }
            }
            Event::Comment(_) => {}
            Event::Decl(decl) => {
                if start != bom_bytes {
                    return Err(invalid(pom, start, "declaration must be first"));
                }
                let value = std::str::from_utf8(decl.as_ref())
                    .map_err(|_| invalid(pom, start, "invalid declaration"))?;
                let tag = BytesStart::from_content(value, 3);
                let mut names = Vec::new();
                for attr in tag.attributes() {
                    let attr =
                        attr.map_err(|_| invalid(pom, start, "invalid declaration attributes"))?;
                    attribute_separator(&tag, attr.key.as_ref(), pom, start)?;
                    match (attr.key.as_ref(), attr.value.as_ref()) {
                        (b"version", b"1.0")
                        | (b"encoding", b"UTF-8" | b"utf-8")
                        | (b"standalone", b"yes" | b"no") => names.push(attr.key.as_ref().to_vec()),
                        _ => {
                            return Err(unsupported(
                                pom,
                                start,
                                "XML declaration outside UTF-8 XML 1.0 subset",
                            ));
                        }
                    }
                }
                if !matches!(names.as_slice(), [v] | [v, _] | [v, _, _] if v == b"version")
                    || names
                        .windows(2)
                        .any(|w| w[0] == b"standalone" || w[1] == b"version")
                {
                    return Err(invalid(pom, start, "invalid declaration order"));
                }
            }
            Event::Eof => {
                if !stack.is_empty() {
                    return Err(invalid(pom, start, "unclosed element"));
                }
                return root.ok_or_else(|| invalid(pom, start, "missing root"));
            }
            _ => {
                return Err(unsupported(
                    pom,
                    start,
                    "DTD, references, CDATA or processing instructions",
                ));
            }
        }
    }
    unreachable!()
}
fn fields(node: &Node, allowed: &[&str], repeated: bool, pom: &str) -> Result<(), Error> {
    if !node.text.chars().all(whitespace) {
        return Err(invalid(pom, node.range.start, "container has text"));
    }
    let mut seen = HashSet::new();
    for child in &node.children {
        if !allowed.contains(&child.name.as_str()) {
            return Err(unsupported(
                pom,
                child.range.start,
                "field outside static Maven subset",
            ));
        }
        if !repeated && !seen.insert(child.name.as_str()) {
            return Err(invalid(pom, child.range.start, "duplicate field"));
        }
    }
    Ok(())
}
fn field<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.children.iter().find(|n| n.name == name)
}
fn scalar<'a>(node: &'a Node, pom: &str) -> Result<&'a str, Error> {
    if !node.children.is_empty() {
        return Err(invalid(pom, node.range.start, "scalar has child elements"));
    }
    let value = node.text.trim_matches(whitespace);
    if value.is_empty() {
        return Err(invalid(pom, node.range.start, "empty scalar"));
    }
    if value.contains("${") {
        return Err(unsupported(pom, node.range.start, "interpolation"));
    }
    Ok(value)
}
fn required<'a>(node: &'a Node, name: &str, pom: &str) -> Result<&'a str, Error> {
    scalar(
        field(node, name)
            .ok_or_else(|| invalid(pom, node.range.start, "missing required literal field"))?,
        pom,
    )
}
fn coordinate(node: &Node, pom: &str) -> Result<Coordinate, Error> {
    let values = [
        required(node, "groupId", pom)?,
        required(node, "artifactId", pom)?,
        required(node, "version", pom)?,
    ];
    for value in values {
        if matches!(value, "." | ".." | "LATEST" | "RELEASE")
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        {
            return Err(unsupported(
                pom,
                node.range.start,
                "requires fixed literal coordinates",
            ));
        }
    }
    let version = values[2].to_ascii_uppercase();
    if version.ends_with("SNAPSHOT") || matches!(version.as_str(), "LATEST" | "RELEASE") {
        return Err(unsupported(
            pom,
            node.range.start,
            "requires fixed literal version",
        ));
    }
    Ok(Coordinate {
        group: values[0].into(),
        artifact: values[1].into(),
        version: values[2].into(),
    })
}
fn relative(value: &str, pom: &str, offset: usize) -> Result<(), Error> {
    if value.len() > MAX_PATH_BYTES {
        return Err(Error::Budget("path bytes"));
    }
    if value
        .split('/')
        .any(|c| c.is_empty() || matches!(c, "." | ".."))
        || value.chars().any(|c| {
            c.is_control() || matches!(c, '\\' | ':' | '$' | '<' | '>' | '*' | '?' | '|' | '"')
        })
    {
        return Err(invalid(
            pom,
            offset,
            "requires contained literal relative path",
        ));
    }
    Ok(())
}
fn joined(base: &Path, relative: &Path, total: &mut usize) -> Result<PathBuf, Error> {
    let length = base
        .as_os_str()
        .len()
        .saturating_add(1)
        .saturating_add(relative.as_os_str().len());
    if length > MAX_PATH_BYTES {
        return Err(Error::Budget("expanded path bytes"));
    }
    charge_path(length, total)?;
    Ok(base.join(relative))
}
fn charge_path(length: usize, total: &mut usize) -> Result<(), Error> {
    if total.saturating_add(length) > MAX_TOTAL_PATH_BYTES {
        return Err(Error::Budget("total expanded path bytes"));
    }
    *total += length;
    Ok(())
}
fn source(pom: &Path, node: Option<&Node>, total: &mut usize) -> Result<Source, Error> {
    charge_path(pom.as_os_str().len(), total)?;
    Ok(Source {
        pom: pom.to_owned(),
        range: node.map(|n| n.range.clone()),
    })
}

/// No I/O or effective-model evaluation. Unsupported input yields no partial result.
pub fn from_poms(base: impl AsRef<Path>, poms: &[Pom<'_>]) -> Result<ImportedMaven, Error> {
    if poms.len() > MAX_POMS {
        return Err(Error::Budget("POM count"));
    }
    let base = base.as_ref();
    if base.as_os_str().len() > MAX_PATH_BYTES {
        return Err(Error::Budget("caller path bytes"));
    }
    if !base.is_absolute()
        || base.to_str().is_none()
        || base
            .components()
            .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
        || base.as_os_str().as_encoded_bytes().contains(&0)
    {
        return Err(invalid(
            "",
            0,
            "base must be a normalized absolute UTF-8 non-NUL path",
        ));
    }
    let normalized_base: PathBuf = base.components().collect();
    let base = normalized_base.as_path();
    let mut bytes = 0usize;
    let mut inputs = HashMap::new();
    for pom in poms {
        if pom.bytes.len() > MAX_POM_BYTES {
            return Err(Error::Budget("POM bytes"));
        }
        bytes += pom.bytes.len();
        if bytes > MAX_TOTAL_BYTES {
            return Err(Error::Budget("total POM bytes"));
        }
        relative(pom.path, pom.path, 0)?;
        if Path::new(pom.path)
            .file_name()
            .is_none_or(|v| v != "pom.xml")
        {
            return Err(invalid(pom.path, 0, "POM key must end in pom.xml"));
        }
        if inputs.insert(pom.path, pom.bytes).is_some() {
            return Err(invalid(pom.path, 0, "duplicate POM key"));
        }
    }
    let mut pending = vec!["pom.xml".to_owned()];
    let mut seen = HashSet::new();
    let mut coordinates = HashSet::new();
    let mut modules = Vec::new();
    let mut total_paths = 0usize;
    while let Some(key) = pending.pop() {
        let key_str = key.as_str();
        if !seen.insert(key.clone()) {
            return Err(invalid(key_str, 0, "duplicate or cyclic module link"));
        }
        let bytes = inputs
            .get(key_str)
            .ok_or_else(|| invalid(key_str, 0, "missing explicitly listed POM"))?;
        let text =
            std::str::from_utf8(bytes).map_err(|_| invalid(key_str, 0, "POM is not UTF-8"))?;
        let root = parse(key_str, text)?;
        if root.name != "project" {
            return Err(invalid(key_str, root.range.start, "expected project root"));
        }
        fields(
            &root,
            &[
                "modelVersion",
                "groupId",
                "artifactId",
                "version",
                "packaging",
                "modules",
                "dependencies",
                "build",
            ],
            false,
            key_str,
        )?;
        if required(&root, "modelVersion", key_str)? != "4.0.0" {
            return Err(unsupported(key_str, 0, "unsupported modelVersion"));
        }
        let coords = coordinate(&root, key_str)?;
        if !coordinates.insert(coords.clone()) {
            return Err(invalid(key_str, 0, "duplicate reactor coordinate"));
        }
        let packaging = field(&root, "packaging")
            .map(|n| scalar(n, key_str))
            .transpose()?
            .unwrap_or("jar");
        if !matches!(packaging, "jar" | "pom") {
            return Err(unsupported(key_str, 0, "unsupported packaging"));
        }
        let pom_path = joined(base, Path::new(&key), &mut total_paths)?;
        let directory = pom_path
            .parent()
            .ok_or_else(|| invalid(key_str, 0, "POM has no parent"))?;
        let mut source_roots = Vec::new();
        let build = field(&root, "build");
        if let Some(build) = build {
            fields(
                build,
                &["sourceDirectory", "testSourceDirectory"],
                false,
                key_str,
            )?;
        }
        if packaging == "pom" && build.is_some_and(|n| !n.children.is_empty()) {
            return Err(unsupported(key_str, 0, "pom packaging source roots"));
        }
        if packaging == "jar" {
            for (name, fallback, kind) in [
                (
                    "sourceDirectory",
                    "src/main/java",
                    SourceRootKind::Production,
                ),
                ("testSourceDirectory", "src/test/java", SourceRootKind::Test),
            ] {
                let node = build.and_then(|b| field(b, name));
                let value = node
                    .map(|n| scalar(n, key_str))
                    .transpose()?
                    .unwrap_or(fallback);
                relative(value, key_str, node.map_or(0, |n| n.range.start))?;
                source_roots.push(SourceRoot {
                    path: joined(directory, Path::new(value), &mut total_paths)?,
                    kind,
                    source: source(&pom_path, node, &mut total_paths)?,
                });
            }
        }
        let mut dependencies = Vec::new();
        if let Some(list) = field(&root, "dependencies") {
            fields(list, &["dependency"], true, key_str)?;
            for dependency in &list.children {
                fields(
                    dependency,
                    &[
                        "groupId",
                        "artifactId",
                        "version",
                        "scope",
                        "type",
                        "optional",
                    ],
                    false,
                    key_str,
                )?;
                let scope_node = field(dependency, "scope");
                let scope = match scope_node
                    .map(|n| scalar(n, key_str))
                    .transpose()?
                    .unwrap_or("compile")
                {
                    "compile" => DependencyScope::Compile,
                    "test" => DependencyScope::Test,
                    "runtime" => DependencyScope::Runtime,
                    "provided" => DependencyScope::Provided,
                    _ => {
                        return Err(unsupported(
                            key_str,
                            dependency.range.start,
                            "unsupported dependency scope",
                        ));
                    }
                };
                for (name, neutral) in [("type", "jar"), ("optional", "false")] {
                    if let Some(node) = field(dependency, name)
                        && scalar(node, key_str)? != neutral
                    {
                        return Err(unsupported(
                            key_str,
                            node.range.start,
                            "unsupported dependency type/optional",
                        ));
                    }
                }
                dependencies.push(UnresolvedDependency {
                    coordinate: coordinate(dependency, key_str)?,
                    scope,
                    scope_defaulted: scope_node.is_none(),
                    source: source(&pom_path, Some(dependency), &mut total_paths)?,
                });
            }
        }
        if let Some(list) = field(&root, "modules") {
            fields(list, &["module"], true, key_str)?;
            if packaging != "pom" && !list.children.is_empty() {
                return Err(unsupported(
                    key_str,
                    list.range.start,
                    "reactor requires pom packaging",
                ));
            }
            for module in list.children.iter().rev() {
                let value = scalar(module, key_str)?;
                relative(value, key_str, module.range.start)?;
                let prefix = key.rsplit_once('/').map_or("", |(prefix, _)| prefix);
                let length =
                    prefix.len() + usize::from(!prefix.is_empty()) + value.len() + "/pom.xml".len();
                if length > MAX_PATH_BYTES {
                    return Err(Error::Budget("module key bytes"));
                }
                if pending.len() + seen.len() >= MAX_POMS {
                    return Err(Error::Budget("reactor links"));
                }
                let child = if prefix.is_empty() {
                    format!("{value}/pom.xml")
                } else {
                    format!("{prefix}/{value}/pom.xml")
                };
                pending.push(child);
            }
        }
        modules.push(Module {
            coordinate: coords,
            packaging: packaging.into(),
            source: source(&pom_path, Some(&root), &mut total_paths)?,
            source_roots,
            dependencies,
        });
    }
    if seen.len() != inputs.len() {
        return Err(invalid("", 0, "unreachable supplied POM"));
    }
    let native_modules: Vec<_> = modules.iter().map(|module| json!({
        "id": format!("{}:{}:{}", module.coordinate.group, module.coordinate.artifact, module.coordinate.version),
        "sourceRoots": module.source_roots.iter().map(|root| json!({"path": root.path, "kind": root.kind})).collect::<Vec<_>>()
    })).collect();
    let mut output = Projection {
        bytes: Vec::new(),
        exceeded: false,
    };
    serde_json::to_writer(
        &mut output,
        &json!({"schemaVersion":1,"modules":native_modules}),
    )
    .map_err(|error| {
        if output.exceeded {
            Error::Budget("projection bytes")
        } else {
            Error::Projection(error)
        }
    })?;
    let project =
        ktlsp_project::from_json(base.join("pom.xml"), &output.bytes).map_err(Error::Project)?;
    Ok(ImportedMaven { project, modules })
}
struct Projection {
    bytes: Vec<u8>,
    exceeded: bool,
}
impl io::Write for Projection {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > ktlsp_project::MAX_MANIFEST_BYTES {
            self.exceeded = true;
            return Err(io::Error::other("projection byte limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    use io::Write;

    #[test]
    fn exact_retained_path_budget_rejects_next_byte_without_charging_it() {
        let mut total = 0;
        charge_path(MAX_TOTAL_PATH_BYTES, &mut total).unwrap();
        assert!(matches!(
            charge_path(1, &mut total),
            Err(Error::Budget("total expanded path bytes"))
        ));
        assert_eq!(total, MAX_TOTAL_PATH_BYTES);
    }

    #[test]
    fn exact_projection_limit_rejects_next_byte_before_retention() {
        let mut projection = Projection {
            bytes: Vec::new(),
            exceeded: false,
        };
        projection
            .write_all(&vec![b' '; ktlsp_project::MAX_MANIFEST_BYTES])
            .unwrap();
        assert!(projection.write_all(b" ").is_err());
        assert!(projection.exceeded);
        assert_eq!(projection.bytes.len(), ktlsp_project::MAX_MANIFEST_BYTES);
    }
}
