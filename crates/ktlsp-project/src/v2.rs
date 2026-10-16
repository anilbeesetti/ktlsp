//! Explicit project-model v2: ordered, scoped/exported module and library edges.
//! Data only; no implicit conversion to v1 or filesystem discovery.

use super::{DependencyScope, LanguageSettings, Library, SourceRoot};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Conservative byte bound before joining caller-supplied bases to model paths.
pub const MAX_EXPANDED_PATH_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_SDKS: usize = 8_192;
pub const MAX_SDK_ROOTS: usize = 32_768;
pub const MAX_SDK_DATA_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug)]
pub enum Error {
    Validation(super::ProjectError),
    UnsupportedSchemaVersion(u64),
    ExpandedPathBudgetExceeded { limit: usize },
    NormalizedOutputBudgetExceeded { limit: usize },
    InvalidSdk { field: &'static str },
    SdkBudgetExceeded { budget: &'static str, limit: usize },
}

impl From<super::ProjectError> for Error {
    fn from(error: super::ProjectError) -> Self {
        Self::Validation(error)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => error.fmt(f),
            Self::UnsupportedSchemaVersion(version) => {
                write!(
                    f,
                    "unsupported project schema version {version}; expected 2"
                )
            }
            Self::ExpandedPathBudgetExceeded { limit } => {
                write!(f, "project expanded path byte budget exceeds {limit}")
            }
            Self::NormalizedOutputBudgetExceeded { limit } => {
                write!(f, "project normalized output byte budget exceeds {limit}")
            }
            Self::InvalidSdk { field } => write!(f, "invalid SDK {field}"),
            Self::SdkBudgetExceeded { budget, limit } => {
                write!(f, "project SDK {budget} budget exceeds {limit}")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            Self::UnsupportedSchemaVersion(_)
            | Self::ExpandedPathBudgetExceeded { .. }
            | Self::NormalizedOutputBudgetExceeded { .. }
            | Self::InvalidSdk { .. }
            | Self::SdkBudgetExceeded { .. } => None,
        }
    }
}

/// Paths returned by the loaders are absolute, Unicode and lexical.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModel {
    pub schema_version: u64,
    pub modules: Vec<Module>,
    pub libraries: Vec<Library>,
    pub sdk_archives: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sdks: Vec<Sdk>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Module {
    pub id: String,
    #[serde(default)]
    pub source_roots: Vec<SourceRoot>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub language_settings: LanguageSettings,
}

/// List order is retained across both variants. Duplicate identity includes
/// variant, target, scope and exported; distinct tuples are never coalesced.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Dependency {
    Module {
        module: String,
        scope: DependencyScope,
        #[serde(default)]
        exported: bool,
    },
    Library {
        library: String,
        scope: DependencyScope,
        #[serde(default)]
        exported: bool,
    },
    Sdk {
        name: String,
        kind: String,
    },
    #[serde(rename = "inheritedSdk")]
    InheritedSdk {},
}

/// Pinned SDK catalog record. Home spelling and URLs are data, not discovered paths.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sdk {
    pub name: String,
    #[serde(rename = "type")]
    pub sdk_type: String,
    #[serde(deserialize_with = "required_nullable")]
    pub version: Option<String>,
    #[serde(deserialize_with = "required_nullable")]
    pub home_path: Option<String>,
    #[serde(default)]
    pub roots: Option<Vec<SdkRoot>>,
    pub additional_data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SdkRoot {
    pub url: String,
    #[serde(rename = "type")]
    pub root_type: String,
}

fn required_nullable<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::deserialize(deserializer)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema_version: u64,
    modules: Vec<Module>,
    #[serde(default)]
    libraries: Vec<Library>,
    #[serde(default)]
    sdk_archives: Vec<PathBuf>,
    #[serde(default)]
    sdks: Vec<Sdk>,
}

/// Reads at most the shared byte limit plus one sentinel byte.
pub fn load_manifest(manifest_path: impl AsRef<Path>) -> Result<ProjectModel, Error> {
    let path = manifest_path.as_ref();
    unicode_path(path)?;
    let file = File::open(path).map_err(super::ProjectError::Io)?;
    let mut bytes = Vec::new();
    file.take((super::MAX_MANIFEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(super::ProjectError::Io)?;
    from_json(path, &bytes)
}

/// Applies aggregate budgets before typed allocation, then validates the whole
/// graph. Relative paths use the manifest parent, just as in v1.
pub fn from_json(manifest_path: impl AsRef<Path>, bytes: &[u8]) -> Result<ProjectModel, Error> {
    if bytes.len() > super::MAX_MANIFEST_BYTES {
        return Err(super::ProjectError::BudgetExceeded {
            budget: super::Budget::ManifestBytes,
            limit: super::MAX_MANIFEST_BYTES,
        }
        .into());
    }
    super::preflight(bytes)?;
    let mut manifest: Manifest =
        serde_json::from_slice(bytes).map_err(super::ProjectError::Json)?;
    if manifest.schema_version != 2 {
        return Err(Error::UnsupportedSchemaVersion(manifest.schema_version));
    }
    let sdks = validate_sdks(&manifest.sdks)?;
    let path = manifest_path.as_ref();
    unicode_path(path)?;
    let absolute = if path.is_absolute() {
        super::normalize(path)
    } else {
        let current_dir = std::env::current_dir().map_err(super::ProjectError::Io)?;
        unicode_path(&current_dir)?;
        super::normalize(&current_dir.join(path))
    };
    unicode_path(&absolute)?;
    let base = absolute
        .parent()
        .filter(|_| absolute.is_absolute())
        .ok_or_else(|| super::ProjectError::InvalidPath(path.to_string_lossy().into_owned()))?;
    validate_path_budget(&manifest, base)?;
    validate_graph(&manifest, sdks)?;

    // Move only shared root/settings/archive data through the existing validator;
    // v2 edges stay untouched and never become v1 dependency/library references.
    let mut shared = super::Manifest {
        schema_version: 2,
        modules: manifest
            .modules
            .iter_mut()
            .map(|module| super::Module {
                id: module.id.clone(),
                source_roots: std::mem::take(&mut module.source_roots),
                dependencies: Vec::new(),
                libraries: Vec::new(),
                language_settings: std::mem::take(&mut module.language_settings),
            })
            .collect(),
        libraries: manifest.libraries,
        sdk_archives: manifest.sdk_archives,
    };
    super::validate_model(&mut shared, base)?;
    for (module, normalized) in manifest.modules.iter_mut().zip(shared.modules) {
        module.source_roots = normalized.source_roots;
        module.language_settings = normalized.language_settings;
    }
    let model = ProjectModel {
        schema_version: manifest.schema_version,
        modules: manifest.modules,
        libraries: shared.libraries,
        sdk_archives: shared.sdk_archives,
        sdks: manifest.sdks,
    };
    let mut output = OutputBudget(0);
    if let Err(error) = serde_json::to_writer(&mut output, &model) {
        if output.0 > super::MAX_MANIFEST_BYTES {
            return Err(Error::NormalizedOutputBudgetExceeded {
                limit: super::MAX_MANIFEST_BYTES,
            });
        }
        return Err(super::ProjectError::Json(error).into());
    }
    Ok(model)
}

// Count the compact serialized bytes, including escaping, without storing output.
struct OutputBudget(usize);
impl Write for OutputBudget {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self.0.saturating_add(bytes.len());
        if self.0 > super::MAX_MANIFEST_BYTES {
            return Err(io::Error::other(
                "normalized project output budget exceeded",
            ));
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn unicode_path(path: &Path) -> Result<(), Error> {
    if path.as_os_str().as_encoded_bytes().len() > MAX_EXPANDED_PATH_BYTES {
        return Err(Error::ExpandedPathBudgetExceeded {
            limit: MAX_EXPANDED_PATH_BYTES,
        });
    }
    super::validate_path(path)?;
    if path.to_str().is_none() {
        return Err(super::ProjectError::InvalidPath(path.to_string_lossy().into_owned()).into());
    }
    Ok(())
}

fn validate_path_budget(manifest: &Manifest, base: &Path) -> Result<(), Error> {
    let base_bytes = base.as_os_str().as_encoded_bytes().len();
    let paths = manifest
        .modules
        .iter()
        .flat_map(|m| m.source_roots.iter().map(|r| &r.path))
        .chain(
            manifest
                .libraries
                .iter()
                .flat_map(|l| l.binaries.iter().chain(&l.sources)),
        )
        .chain(&manifest.sdk_archives);
    let mut expanded = 0usize;
    for path in paths {
        let bytes = path.as_os_str().as_encoded_bytes().len();
        expanded = expanded.saturating_add(
            if path.is_absolute() && !super::retains_base_prefix(base, path) {
                bytes
            } else {
                base_bytes.saturating_add(1).saturating_add(bytes)
            },
        );
        if expanded > MAX_EXPANDED_PATH_BYTES {
            return Err(Error::ExpandedPathBudgetExceeded {
                limit: MAX_EXPANDED_PATH_BYTES,
            });
        }
    }
    Ok(())
}

fn validate_graph(
    manifest: &Manifest,
    sdks: HashSet<(&str, &str)>,
) -> Result<(), super::ProjectError> {
    let modules = super::ids(manifest.modules.iter().map(|m| m.id.as_str()), "module")?;
    let libraries = super::ids(manifest.libraries.iter().map(|l| l.id.as_str()), "library")?;
    let mut graph = vec![Vec::new(); manifest.modules.len()];
    let mut indegree = vec![0usize; manifest.modules.len()];
    for (index, module) in manifest.modules.iter().enumerate() {
        let mut seen = HashSet::new();
        let mut sdk_edges = HashSet::new();
        let mut inherited_sdk = false;
        for dependency in &module.dependencies {
            match dependency {
                Dependency::Module {
                    module: target,
                    scope,
                    exported,
                } => {
                    let &target_index = modules.get(target.as_str()).ok_or_else(|| {
                        super::ProjectError::UnknownReference {
                            kind: "module",
                            id: target.clone(),
                        }
                    })?;
                    if index == target_index {
                        return Err(super::ProjectError::SelfDependency(module.id.clone()));
                    }
                    if !seen.insert((false, target.as_str(), *scope as u8, *exported)) {
                        return Err(super::ProjectError::DuplicateDependency {
                            module: module.id.clone(),
                            dependency: target.clone(),
                        });
                    }
                    graph[index].push(target_index);
                    indegree[target_index] += 1;
                }
                Dependency::Library {
                    library,
                    scope,
                    exported,
                } => {
                    if !libraries.contains_key(library.as_str()) {
                        return Err(super::ProjectError::UnknownReference {
                            kind: "library",
                            id: library.clone(),
                        });
                    }
                    if !seen.insert((true, library.as_str(), *scope as u8, *exported)) {
                        return Err(super::ProjectError::DuplicateLibraryReference {
                            module: module.id.clone(),
                            library: library.clone(),
                        });
                    }
                }
                Dependency::Sdk { name, kind } => {
                    if !sdks.contains(&(name.as_str(), kind.as_str())) {
                        return Err(super::ProjectError::UnknownReference {
                            kind: "SDK",
                            id: format!("{name:?}/{kind:?}"),
                        });
                    }
                    if !sdk_edges.insert((name, kind)) {
                        return Err(super::ProjectError::DuplicateDependency {
                            module: module.id.clone(),
                            dependency: format!("SDK {name:?}/{kind:?}"),
                        });
                    }
                }
                Dependency::InheritedSdk {} => {
                    if std::mem::replace(&mut inherited_sdk, true) {
                        return Err(super::ProjectError::DuplicateDependency {
                            module: module.id.clone(),
                            dependency: "inheritedSdk".into(),
                        });
                    }
                }
            }
        }
    }
    let mut ready: Vec<_> = indegree
        .iter()
        .enumerate()
        .filter_map(|(index, count)| (*count == 0).then_some(index))
        .collect();
    let mut visited = 0;
    while let Some(index) = ready.pop() {
        visited += 1;
        for &target in &graph[index] {
            indegree[target] -= 1;
            if indegree[target] == 0 {
                ready.push(target);
            }
        }
    }
    if visited != manifest.modules.len() {
        return Err(super::ProjectError::DependencyCycle);
    }
    Ok(())
}

fn sdk_text(value: &str, field: &'static str, empty: bool) -> Result<(), Error> {
    if (!empty && value.is_empty()) || value.contains('\0') {
        return Err(Error::InvalidSdk { field });
    }
    Ok(())
}

fn validate_sdks(sdks: &[Sdk]) -> Result<HashSet<(&str, &str)>, Error> {
    let mut identities = HashSet::new();
    let mut bytes = 0usize;
    let mut roots = 0usize;
    for sdk in sdks {
        sdk_text(&sdk.name, "name", false)?;
        sdk_text(&sdk.sdk_type, "type", false)?;
        if !identities.insert((sdk.name.as_str(), sdk.sdk_type.as_str())) {
            return Err(super::ProjectError::DuplicateId {
                kind: "SDK",
                id: format!("{:?}/{:?}", sdk.name, sdk.sdk_type),
            }
            .into());
        }
        bytes = bytes
            .saturating_add(sdk.name.len())
            .saturating_add(sdk.sdk_type.len())
            .saturating_add(sdk.additional_data.len());
        sdk_text(&sdk.additional_data, "additionalData", true)?;
        if let Some(version) = &sdk.version {
            sdk_text(version, "version", true)?;
            bytes = bytes.saturating_add(version.len());
        }
        if let Some(home) = &sdk.home_path {
            sdk_text(home, "homePath", false)?;
            // ponytail: retain supported lexical spelling; filesystem/foreign-platform resolution is separate.
            let workspace = home.strip_prefix("<WORKSPACE>/");
            if home.contains('\\')
                || home.contains("://")
                || workspace.unwrap_or(home).contains('<')
                || workspace.is_some_and(|rest| rest.starts_with('/'))
                || (workspace.is_none() && !Path::new(home).is_absolute())
            {
                return Err(Error::InvalidSdk { field: "homePath" });
            }
            bytes = bytes.saturating_add(home.len());
        }
        for root in sdk.roots.iter().flatten() {
            sdk_text(&root.root_type, "root type", false)?;
            sdk_text(&root.url, "root URL", false)?;
            let Some((scheme, rest)) = root.url.split_once("://") else {
                return Err(Error::InvalidSdk { field: "root URL" });
            };
            if rest.is_empty()
                || !scheme
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphabetic)
                || !scheme
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
                || root
                    .url
                    .chars()
                    .any(|c| c.is_whitespace() || c.is_control() || c == '\\')
            {
                return Err(Error::InvalidSdk { field: "root URL" });
            }
            roots = roots.saturating_add(1);
            bytes = bytes
                .saturating_add(root.url.len())
                .saturating_add(root.root_type.len());
        }
    }
    for (count, budget, limit) in [
        (sdks.len(), "catalog", MAX_SDKS),
        (roots, "roots", MAX_SDK_ROOTS),
        (bytes, "data bytes", MAX_SDK_DATA_BYTES),
    ] {
        if count > limit {
            return Err(Error::SdkBudgetExceeded { budget, limit });
        }
    }
    Ok(identities)
}
