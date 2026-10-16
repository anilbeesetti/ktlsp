//! Bounded, data-only import of a pinned official workspace.json subset.
//! The complete result includes ordered import metadata; its v1 projection alone
//! does not preserve content-root boundaries, source types or classpath order.

mod json;
pub mod maven;

use ktlsp_project::{DependencyScope, ModuleDependency, ProjectError, ProjectModel};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use std::fmt;
use std::path::{Component, Path, PathBuf};

pub const OFFICIAL_SOURCE: &str = "4acdb6ec74bbcfe0d30f7ecbcae053bf729b1748";
pub const MAX_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_NODES: usize = 100_000;
pub const MAX_DEPTH: usize = 32;
pub const MAX_REPORTS: usize = 128;
pub const MAX_EXPANDED_PATH_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedField {
    /// JSON pointer into the original document.
    pub path: String,
    pub reason: &'static str,
}

#[derive(Debug)]
pub enum ImportError {
    Json(serde_json::Error),
    BudgetExceeded(&'static str),
    Schema {
        path: String,
        message: String,
    },
    Unsupported {
        fields: Vec<UnsupportedField>,
        truncated: bool,
    },
    Project(ProjectError),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(e) => write!(f, "invalid workspace JSON: {e}"),
            Self::BudgetExceeded(b) => write!(f, "workspace {b} budget exceeded"),
            Self::Schema { path, message } => {
                write!(f, "invalid workspace field {path}: {message}")
            }
            Self::Unsupported { fields, truncated } => {
                write!(f, "unsupported workspace semantics")?;
                for field in fields {
                    write!(f, "; {}: {}", field.path, field.reason)?;
                }
                if *truncated {
                    write!(f, "; further fields omitted")?;
                }
                Ok(())
            }
            Self::Project(e) => write!(f, "incompatible workspace project: {e}"),
        }
    }
}
impl std::error::Error for ImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(e) => Some(e),
            Self::Project(e) => Some(e),
            _ => None,
        }
    }
}

/// Supported import contract. Use ordered metadata as well as the v1 projection.
#[derive(Debug)]
pub struct ImportedWorkspace {
    project: ProjectModel,
    pub modules: Vec<ImportedModule>,
    pub libraries: Vec<ImportedLibrary>,
}
impl ImportedWorkspace {
    /// Validated v1 projection, not the complete imported workspace semantics.
    pub fn project_projection(&self) -> &ProjectModel {
        &self.project
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct ImportedModule {
    pub name: String,
    pub content_roots: Vec<ContentRoot>,
    pub dependencies: Vec<Dependency>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ContentRoot {
    pub path: PathBuf,
    pub source_roots: Vec<SourceRoot>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct SourceRoot {
    pub path: PathBuf,
    pub root_type: String,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Dependency {
    Module(ModuleDependency),
    Library(String),
    ModuleSource,
}
#[derive(Debug, PartialEq, Eq)]
pub struct ImportedLibrary {
    pub name: String,
    pub roots: Vec<LibraryRoot>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct LibraryRoot {
    pub path: PathBuf,
    pub root_type: String,
}

fn field_path(base: &str, key: &str) -> String {
    format!("{base}/{}", key.replace('~', "~0").replace('/', "~1"))
}
fn schema(path: &str, message: impl ToString) -> ImportError {
    ImportError::Schema {
        path: path.into(),
        message: message.to_string(),
    }
}
fn object(value: Value, path: &str) -> Result<Map<String, Value>, ImportError> {
    match value {
        Value::Object(o) => Ok(o),
        _ => Err(schema(path, "expected object")),
    }
}
fn required<T: DeserializeOwned>(
    o: &mut Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<T, ImportError> {
    let p = field_path(path, key);
    let value = o
        .remove(key)
        .ok_or_else(|| schema(&p, "missing required field"))?;
    serde_json::from_value(value).map_err(|e| schema(&p, e))
}
fn default<T: DeserializeOwned>(
    o: &mut Map<String, Value>,
    key: &str,
    path: &str,
    fallback: T,
) -> Result<T, ImportError> {
    if o.contains_key(key) {
        required(o, key, path)
    } else {
        Ok(fallback)
    }
}
#[derive(Default)]
struct Reports {
    expanded_path_bytes: usize,
    fields: Vec<UnsupportedField>,
    truncated: bool,
}
impl Reports {
    fn add(&mut self, path: String, reason: &'static str) {
        if self.fields.len() < MAX_REPORTS {
            self.fields.push(UnsupportedField { path, reason });
        } else {
            self.truncated = true;
        }
    }
    fn neutral(
        &mut self,
        o: &mut Map<String, Value>,
        key: &str,
        neutral: Value,
        path: &str,
        reason: &'static str,
    ) {
        if let Some(value) = o.remove(key)
            && value != neutral
        {
            self.add(field_path(path, key), reason);
        }
    }
    fn unknown(&mut self, o: Map<String, Value>, path: &str) {
        for key in o.keys() {
            self.add(field_path(path, key), "unknown field in pinned subset");
        }
    }
}

// shortcut: lexical host paths only; add explicit filesystem/symlink import policy before official path parity.
fn path(
    value: String,
    location: &str,
    base: &Path,
    reports: &mut Reports,
) -> Result<PathBuf, ImportError> {
    if value.is_empty() || value.contains('\0') {
        return Err(schema(location, "empty or NUL path"));
    }
    let expanded_len = if let Some(rest) = value.strip_prefix("<WORKSPACE>/") {
        base.as_os_str()
            .as_encoded_bytes()
            .len()
            .saturating_add(1)
            .saturating_add(rest.len())
    } else {
        value.len()
    };
    reports.expanded_path_bytes = reports.expanded_path_bytes.saturating_add(expanded_len);
    if reports.expanded_path_bytes > MAX_EXPANDED_PATH_BYTES {
        return Err(ImportError::BudgetExceeded("expanded path bytes"));
    }
    let candidate = if let Some(rest) = value.strip_prefix("<WORKSPACE>/") {
        if rest.starts_with('/') {
            reports.add(location.into(), "invalid workspace macro suffix");
            return Ok(PathBuf::new());
        }
        base.join(rest)
    } else {
        PathBuf::from(&value)
    };
    if value.contains('\\')
        || value.contains("://")
        || value
            .strip_prefix("<WORKSPACE>/")
            .unwrap_or(&value)
            .contains('<')
        || !candidate.is_absolute()
    {
        reports.add(
            location.into(),
            "requires <WORKSPACE>/ or an absolute host path; no URI, foreign path or other macro",
        );
        return Ok(PathBuf::new());
    }
    let mut normalized = PathBuf::new();
    for component in candidate.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    Ok(normalized)
}

/// `project_directory` must be absolute; no file I/O, environment macros or build execution.
/// Unsupported fields return no usable partial model. The official format has no version field.
pub fn from_json(
    project_directory: impl AsRef<Path>,
    bytes: &[u8],
) -> Result<ImportedWorkspace, ImportError> {
    let base = project_directory.as_ref();
    let parsed = parse_workspace(base, bytes, false)?;
    let bytes = serde_json::to_vec(&parsed.manifest).map_err(ImportError::Json)?;
    let project = ktlsp_project::from_json(base.join("workspace.json"), &bytes)
        .map_err(ImportError::Project)?;
    let modules = parsed
        .modules
        .into_iter()
        .map(|module| {
            Ok(ImportedModule {
                name: module.name,
                content_roots: module.content_roots,
                dependencies: module
                    .dependencies
                    .into_iter()
                    .map(|edge| match edge {
                        v2::Dependency::Module(edge) => Ok(Dependency::Module(edge)),
                        v2::Dependency::Library { library, .. } => Ok(Dependency::Library(library)),
                        v2::Dependency::ModuleSource => Ok(Dependency::ModuleSource),
                        v2::Dependency::Sdk { .. } | v2::Dependency::InheritedSdk => {
                            Err(schema("", "SDK edges cannot enter a v1 projection"))
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            })
        })
        .collect::<Result<Vec<_>, ImportError>>()?;
    Ok(ImportedWorkspace {
        project,
        modules,
        libraries: parsed.libraries,
    })
}

struct ParsedWorkspace {
    manifest: Value,
    modules: Vec<v2::ImportedModule>,
    libraries: Vec<ImportedLibrary>,
}

fn parse_workspace(
    base: &Path,
    bytes: &[u8],
    use_v2: bool,
) -> Result<ParsedWorkspace, ImportError> {
    if bytes.len() > MAX_BYTES {
        return Err(ImportError::BudgetExceeded("bytes"));
    }
    if base.as_os_str().as_encoded_bytes().len() > MAX_EXPANDED_PATH_BYTES {
        return Err(ImportError::BudgetExceeded("project directory bytes"));
    }
    if !base.is_absolute()
        || base.to_str().is_none()
        || base.as_os_str().as_encoded_bytes().contains(&0)
    {
        return Err(schema(
            "",
            "project directory must be an absolute UTF-8 non-NUL path",
        ));
    }
    let mut workspace = object(json::parse(bytes)?, "")?;
    let mut reports = Reports::default();
    let sdks = if use_v2 {
        v2::parse_sdks(&mut workspace, &mut reports)?
    } else {
        reports.neutral(
            &mut workspace,
            "sdks",
            json!([]),
            "",
            "SDK catalog requires v2",
        );
        Vec::new()
    };
    for key in ["kotlinSettings", "javaSettings"] {
        reports.neutral(
            &mut workspace,
            key,
            json!([]),
            "",
            "SDK/compiler settings require a richer native model",
        );
    }
    reports.neutral(
        &mut workspace,
        "externalSystem",
        Value::Null,
        "",
        "build-system provenance is unsupported",
    );
    let module_values: Vec<Value> = default(&mut workspace, "modules", "", vec![])?;
    let library_values: Vec<Value> = default(&mut workspace, "libraries", "", vec![])?;
    reports.unknown(workspace, "");
    let mut native_modules = Vec::new();
    let mut modules = Vec::new();
    for (i, value) in module_values.into_iter().enumerate() {
        let location = format!("/modules/{i}");
        let mut m = object(value, &location)?;
        let name: String = required(&mut m, "name", &location)?;
        reports.neutral(
            &mut m,
            "type",
            json!("JAVA_MODULE"),
            &location,
            "unsupported module type",
        );
        for key in ["coordinate", "externalProjectPath", "externalProjectId"] {
            reports.neutral(
                &mut m,
                key,
                Value::Null,
                &location,
                "module build metadata is unsupported",
            );
        }
        reports.neutral(
            &mut m,
            "facets",
            json!([]),
            &location,
            "facets/plugins are unsupported",
        );
        let mut native_edges = Vec::new();
        let mut native_libraries = Vec::new();
        let mut dependencies = Vec::new();
        let values: Vec<Value> = default(&mut m, "dependencies", &location, vec![])?;
        for (j, value) in values.into_iter().enumerate() {
            let p = format!("{location}/dependencies/{j}");
            let mut d = object(value, &p)?;
            let kind: String = required(&mut d, "type", &p)?;
            match kind.as_str() {
                "module" | "library" => {
                    let name: String = required(&mut d, "name", &p)?;
                    let scope: DependencyScope = required(&mut d, "scope", &p)?;
                    let exported: bool = default(&mut d, "isExported", &p, false)?;
                    if kind == "module" {
                        reports.neutral(
                            &mut d,
                            "isTestJar",
                            json!(false),
                            &p,
                            "test JAR module edges are unsupported",
                        );
                        let edge = ModuleDependency {
                            module: name,
                            scope,
                            exported,
                        };
                        native_edges.push(if use_v2 {
                            json!({"type": "module", "module": edge.module, "scope": edge.scope, "exported": edge.exported})
                        } else {
                            json!(edge)
                        });
                        dependencies.push(v2::Dependency::Module(edge));
                    } else {
                        if !use_v2 && scope != DependencyScope::Compile {
                            reports.add(
                                field_path(&p, "scope"),
                                "v1 cannot represent scoped library edges",
                            );
                        }
                        if !use_v2 && exported {
                            reports.add(
                                field_path(&p, "isExported"),
                                "v1 cannot represent exported library edges",
                            );
                        }
                        if use_v2 {
                            native_edges.push(json!({"type": "library", "library": name, "scope": scope, "exported": exported}));
                        } else {
                            native_libraries.push(name.clone());
                        }
                        dependencies.push(v2::Dependency::Library {
                            library: name,
                            scope,
                            exported,
                        });
                    }
                }
                "moduleSource" => dependencies.push(v2::Dependency::ModuleSource),
                "sdk" if use_v2 => {
                    let name: String = required(&mut d, "name", &p)?;
                    let kind: String = required(&mut d, "kind", &p)?;
                    native_edges.push(json!({"type":"sdk","name":name,"kind":kind}));
                    dependencies.push(v2::Dependency::Sdk { name, kind });
                }
                "inheritedSdk" if use_v2 => {
                    native_edges.push(json!({"type":"inheritedSdk"}));
                    dependencies.push(v2::Dependency::InheritedSdk);
                }
                _ => reports.add(
                    field_path(&p, "type"),
                    "unsupported dependency variant (including SDK bindings)",
                ),
            }
            reports.unknown(d, &p);
        }
        let values: Vec<Value> = default(&mut m, "contentRoots", &location, vec![])?;
        let mut content_roots = Vec::new();
        let mut native_roots = Vec::new();
        for (j, value) in values.into_iter().enumerate() {
            let p = format!("{location}/contentRoots/{j}");
            let mut c = object(value, &p)?;
            let content_path = path(
                required(&mut c, "path", &p)?,
                &field_path(&p, "path"),
                base,
                &mut reports,
            )?;
            for key in ["excludedPatterns", "excludedUrls"] {
                reports.neutral(
                    &mut c,
                    key,
                    json!([]),
                    &p,
                    "excluded roots/patterns are unsupported",
                );
            }
            let values: Vec<Value> = default(&mut c, "sourceRoots", &p, vec![])?;
            let mut source_roots = Vec::new();
            for (k, value) in values.into_iter().enumerate() {
                let s = format!("{p}/sourceRoots/{k}");
                let mut r = object(value, &s)?;
                let root_path = path(
                    required(&mut r, "path", &s)?,
                    &field_path(&s, "path"),
                    base,
                    &mut reports,
                )?;
                let root_type: String = required(&mut r, "type", &s)?;
                let kind = match root_type.as_str() {
                    "java-source" | "kotlin-source" => "production",
                    "java-test" | "kotlin-test" => "test",
                    _ => {
                        reports.add(
                            field_path(&s, "type"),
                            "resource/generated/custom root types are unsupported",
                        );
                        "production"
                    }
                };
                native_roots.push(json!({"path": root_path, "kind": kind}));
                source_roots.push(SourceRoot {
                    path: root_path,
                    root_type,
                });
                reports.unknown(r, &s);
            }
            content_roots.push(ContentRoot {
                path: content_path,
                source_roots,
            });
            reports.unknown(c, &p);
        }
        native_modules.push(if use_v2 {
            json!({"id": name, "sourceRoots": native_roots, "dependencies": native_edges})
        } else {
            json!({"id": name, "sourceRoots": native_roots, "dependencies": native_edges, "libraries": native_libraries})
        });
        modules.push(v2::ImportedModule {
            name,
            dependencies,
            content_roots,
        });
        reports.unknown(m, &location);
    }
    let mut libraries = Vec::new();
    let mut native_libraries = Vec::new();
    for (i, value) in library_values.into_iter().enumerate() {
        let p = format!("/libraries/{i}");
        let mut l = object(value, &p)?;
        let name: String = required(&mut l, "name", &p)?;
        let library_type: Option<String> = required(&mut l, "type", &p)?;
        if library_type.is_some() {
            reports.add(
                field_path(&p, "type"),
                "special library types are unsupported",
            );
        }
        reports.neutral(
            &mut l,
            "level",
            json!("project"),
            &p,
            "module/application library levels are unsupported",
        );
        for key in ["module", "properties"] {
            reports.neutral(
                &mut l,
                key,
                Value::Null,
                &p,
                "library ownership/properties are unsupported",
            );
        }
        reports.neutral(
            &mut l,
            "excludedRoots",
            json!([]),
            &p,
            "excluded library roots are unsupported",
        );
        let values: Vec<Value> = required(&mut l, "roots", &p)?;
        let mut roots = Vec::new();
        let mut binaries = Vec::new();
        let mut sources = Vec::new();
        for (j, value) in values.into_iter().enumerate() {
            let r = format!("{p}/roots/{j}");
            let mut root = object(value, &r)?;
            let root_path = path(
                required(&mut root, "path", &r)?,
                &field_path(&r, "path"),
                base,
                &mut reports,
            )?;
            let root_type: String = default(&mut root, "type", &r, "CLASSES".into())?;
            match root_type.as_str() {
                "CLASSES" => binaries.push(root_path.clone()),
                "SOURCES" => sources.push(root_path.clone()),
                _ => reports.add(field_path(&r, "type"), "unsupported library root type"),
            }
            reports.neutral(
                &mut root,
                "inclusionOptions",
                json!("root_itself"),
                &r,
                "archive-directory traversal is unsupported",
            );
            roots.push(LibraryRoot {
                path: root_path,
                root_type,
            });
            reports.unknown(root, &r);
        }
        native_libraries.push(json!({"id": name, "binaries": binaries, "sources": sources}));
        libraries.push(ImportedLibrary { name, roots });
        reports.unknown(l, &p);
    }
    if !reports.fields.is_empty() {
        return Err(ImportError::Unsupported {
            fields: reports.fields,
            truncated: reports.truncated,
        });
    }
    let mut manifest = json!({"schemaVersion": if use_v2 { 2 } else { 1 }, "modules": native_modules, "libraries": native_libraries});
    if !sdks.is_empty() {
        manifest["sdks"] = json!(sdks);
    }
    Ok(ParsedWorkspace {
        manifest,
        modules,
        libraries,
    })
}

/// Additive ordered scoped/exported import API; the original v1 API stays strict.
pub mod v2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_intermediate_manifest_matches_frozen_literal() {
        let parsed = parse_workspace(
            Path::new("/ktlsp/t021"),
            include_bytes!("../tests/fixtures/scoped-exported.json"),
            true,
        )
        .unwrap();
        let expected: Value = serde_json::from_slice(include_bytes!(
            "../tests/fixtures/scoped-exported-projection.json"
        ))
        .unwrap();
        assert_eq!(parsed.manifest, expected);
    }
}
