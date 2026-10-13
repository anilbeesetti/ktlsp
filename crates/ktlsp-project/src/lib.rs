//! Pure Rust loader for the explicit ktlsp project-model v1.
//!
//! Paths are normalized lexically, including missing archives. No build tools,
//! downloads, classpath traversal, or symlink resolution occur. Symlink aliases
//! need a separate filesystem ownership check before files are analyzed.

use serde::de::{DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

pub mod v2;

pub const MAX_MANIFEST_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_MODULES: usize = 4_096;
pub const MAX_LIBRARIES: usize = 8_192;
/// Shared budget for module dependency edges and module library references.
pub const MAX_DEPENDENCIES: usize = 65_536;
pub const MAX_SOURCE_ROOTS: usize = 16_384;
pub const MAX_ARCHIVE_PATHS: usize = 32_768;
pub const MAX_CALLER_PATH_BYTES: usize = 8 * 1024 * 1024;
/// Shared conservative prejoin bound across all model path collections.
pub const MAX_EXPANDED_PATH_BYTES: usize = 8 * 1024 * 1024;

/// Resource limits are checked before the related allocation or path expansion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Budget {
    ManifestBytes,
    Modules,
    Libraries,
    Dependencies,
    SourceRoots,
    ArchivePaths,
    CallerPathBytes,
    ExpandedPathBytes,
}

#[derive(Debug)]
pub enum ProjectError {
    Io(io::Error),
    Json(serde_json::Error),
    BudgetExceeded {
        budget: Budget,
        limit: usize,
    },
    UnsupportedSchemaVersion(u64),
    InvalidId {
        kind: &'static str,
        id: String,
    },
    DuplicateId {
        kind: &'static str,
        id: String,
    },
    UnknownReference {
        kind: &'static str,
        id: String,
    },
    SelfDependency(String),
    DuplicateDependency {
        module: String,
        dependency: String,
    },
    DuplicateLibraryReference {
        module: String,
        library: String,
    },
    DependencyCycle,
    InvalidPath(String),
    InvalidVersion {
        field: &'static str,
        value: String,
    },
    ApiVersionExceedsLanguageVersion {
        module: String,
    },
    SourceRootOverlap {
        first_module: String,
        second_module: String,
        first: PathBuf,
        second: PathBuf,
    },
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "project I/O error: {e}"),
            Self::Json(e) => write!(f, "invalid project JSON/schema: {e}"),
            Self::BudgetExceeded { budget, limit } => {
                write!(f, "project {budget:?} budget exceeds {limit}")
            }
            Self::UnsupportedSchemaVersion(v) => {
                write!(f, "unsupported project schema version {v}; expected 1")
            }
            Self::InvalidId { kind, id } => write!(f, "empty {kind} ID: {id:?}"),
            Self::DuplicateId { kind, id } => write!(f, "duplicate {kind} ID: {id:?}"),
            Self::UnknownReference { kind, id } => write!(f, "unknown {kind} reference: {id:?}"),
            Self::SelfDependency(id) => write!(f, "module {id:?} depends on itself"),
            Self::DuplicateDependency { module, dependency } => {
                write!(f, "module {module:?} repeats dependency {dependency:?}")
            }
            Self::DuplicateLibraryReference { module, library } => {
                write!(f, "module {module:?} repeats library reference {library:?}")
            }
            Self::DependencyCycle => write!(f, "module dependency graph contains a cycle"),
            Self::InvalidPath(p) => write!(f, "invalid project path: {p:?}"),
            Self::InvalidVersion { field, value } => write!(f, "invalid {field}: {value:?}"),
            Self::ApiVersionExceedsLanguageVersion { module } => {
                write!(f, "module {module:?} API version exceeds language version")
            }
            Self::SourceRootOverlap {
                first_module,
                second_module,
                first,
                second,
            } => write!(
                f,
                "source roots {} ({first_module}) and {} ({second_module}) have conflicting ownership or semantics",
                first.display(),
                second.display()
            ),
        }
    }
}

impl std::error::Error for ProjectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Json(e) => Some(e),
            _ => None,
        }
    }
}

/// Resolved paths are absolute and lexical; their existence is not implied.
/// Library IDs on modules mean compile/non-exported references in schema v1.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModel {
    pub schema_version: u64,
    pub modules: Vec<Module>,
    pub libraries: Vec<Library>,
    pub sdk_archives: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Module {
    pub id: String,
    #[serde(default)]
    pub source_roots: Vec<SourceRoot>,
    #[serde(default)]
    pub dependencies: Vec<ModuleDependency>,
    #[serde(default)]
    pub libraries: Vec<String>,
    #[serde(default)]
    pub language_settings: LanguageSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceRoot {
    pub path: PathBuf,
    pub kind: SourceRootKind,
    #[serde(default)]
    pub generated: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceRootKind {
    Production,
    Test,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModuleDependency {
    pub module: String,
    pub scope: DependencyScope,
    #[serde(default)]
    pub exported: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DependencyScope {
    Compile,
    Test,
    Runtime,
    Provided,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LanguageSettings {
    pub language_version: Option<String>,
    pub api_version: Option<String>,
    pub jvm_target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Library {
    pub id: String,
    pub binaries: Vec<PathBuf>,
    #[serde(default)]
    pub sources: Vec<PathBuf>,
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
}

/// Reads at most the byte budget plus one sentinel byte, even if the file grows.
pub fn load_manifest(manifest_path: impl AsRef<Path>) -> Result<ProjectModel, ProjectError> {
    let path = manifest_path.as_ref();
    caller_path_bytes(path)?;
    let file = File::open(path).map_err(ProjectError::Io)?;
    let mut bytes = Vec::new();
    file.take((MAX_MANIFEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(ProjectError::Io)?;
    from_json(path, &bytes)
}

/// Parses bytes using the manifest's parent as the relative-path base.
/// Relative manifest paths are interpreted against the current working directory.
pub fn from_json(
    manifest_path: impl AsRef<Path>,
    bytes: &[u8],
) -> Result<ProjectModel, ProjectError> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(ProjectError::BudgetExceeded {
            budget: Budget::ManifestBytes,
            limit: MAX_MANIFEST_BYTES,
        });
    }
    preflight(bytes)?;
    let mut manifest: Manifest = serde_json::from_slice(bytes).map_err(ProjectError::Json)?;
    if manifest.schema_version != 1 {
        return Err(ProjectError::UnsupportedSchemaVersion(
            manifest.schema_version,
        ));
    }
    let path = manifest_path.as_ref();
    let path_bytes = caller_path_bytes(path)?;
    validate_path(path)?;
    let absolute = if path.is_absolute() {
        normalize(path)
    } else {
        let current_dir = std::env::current_dir().map_err(ProjectError::Io)?;
        let cwd_bytes = caller_path_bytes(&current_dir)?;
        check_path_bytes(
            cwd_bytes
                .checked_add(1)
                .and_then(|n| n.checked_add(path_bytes)),
            Budget::CallerPathBytes,
            MAX_CALLER_PATH_BYTES,
        )?;
        normalize(&current_dir.join(path))
    };
    if !absolute.is_absolute() {
        return Err(ProjectError::InvalidPath(
            path.to_string_lossy().into_owned(),
        ));
    }
    let base = absolute
        .parent()
        .ok_or_else(|| ProjectError::InvalidPath(path.to_string_lossy().into_owned()))?;
    validate_path_budget(&manifest, base)?;
    validate_model(&mut manifest, base)?;
    Ok(ProjectModel {
        schema_version: manifest.schema_version,
        modules: manifest.modules,
        libraries: manifest.libraries,
        sdk_archives: manifest.sdk_archives,
    })
}

fn check_path_bytes(
    bytes: Option<usize>,
    budget: Budget,
    limit: usize,
) -> Result<usize, ProjectError> {
    bytes
        .filter(|&n| n <= limit)
        .ok_or(ProjectError::BudgetExceeded { budget, limit })
}

fn caller_path_bytes(path: &Path) -> Result<usize, ProjectError> {
    check_path_bytes(
        Some(path.as_os_str().as_encoded_bytes().len()),
        Budget::CallerPathBytes,
        MAX_CALLER_PATH_BYTES,
    )
}

fn validate_path_budget(manifest: &Manifest, base: &Path) -> Result<(), ProjectError> {
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
    let mut total = 0usize;
    for path in paths {
        let bytes = path.as_os_str().as_encoded_bytes().len();
        let expanded = if path.is_absolute() && !retains_base_prefix(base, path) {
            Some(bytes)
        } else {
            base_bytes.checked_add(1).and_then(|n| n.checked_add(bytes))
        };
        total = check_path_bytes(
            expanded.and_then(|n| total.checked_add(n)),
            Budget::ExpandedPathBytes,
            MAX_EXPANDED_PATH_BYTES,
        )?;
    }
    Ok(())
}

fn validate_path(path: &Path) -> Result<(), ProjectError> {
    // JSON paths are Unicode strings; this check also handles caller-provided OS paths.
    if path.as_os_str().is_empty() || path.as_os_str().as_encoded_bytes().contains(&0) {
        return Err(ProjectError::InvalidPath(
            path.to_string_lossy().into_owned(),
        ));
    }
    Ok(())
}

fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

fn retains_base_prefix(base: &Path, path: &Path) -> bool {
    // Cygwin absolute root-only paths retain the base prefix when joined.
    matches!(path.components().next(), Some(Component::RootDir))
        && matches!(base.components().next(), Some(Component::Prefix(_)))
}

fn resolve(path: &mut PathBuf, base: &Path) -> Result<(), ProjectError> {
    validate_path(path)?;
    let resolved = if path.is_absolute() && !retains_base_prefix(base, path) {
        normalize(path)
    } else {
        normalize(&base.join(&*path))
    };
    if !resolved.is_absolute() {
        return Err(ProjectError::InvalidPath(
            path.to_string_lossy().into_owned(),
        ));
    }
    *path = resolved;
    Ok(())
}

fn ids<'a>(
    values: impl Iterator<Item = &'a str>,
    kind: &'static str,
) -> Result<HashMap<&'a str, usize>, ProjectError> {
    let mut result = HashMap::new();
    for (index, id) in values.enumerate() {
        if id.trim().is_empty() {
            return Err(ProjectError::InvalidId {
                kind,
                id: id.into(),
            });
        }
        if result.insert(id, index).is_some() {
            return Err(ProjectError::DuplicateId {
                kind,
                id: id.into(),
            });
        }
    }
    Ok(result)
}

fn validate_model(manifest: &mut Manifest, base: &Path) -> Result<(), ProjectError> {
    let module_ids = ids(manifest.modules.iter().map(|m| m.id.as_str()), "module")?;
    let library_ids = ids(manifest.libraries.iter().map(|l| l.id.as_str()), "library")?;
    let mut indegree = vec![0usize; manifest.modules.len()];
    let mut graph = vec![Vec::new(); manifest.modules.len()];
    for (index, module) in manifest.modules.iter().enumerate() {
        let mut targets = HashSet::new();
        for dependency in &module.dependencies {
            let &target = module_ids.get(dependency.module.as_str()).ok_or_else(|| {
                ProjectError::UnknownReference {
                    kind: "module",
                    id: dependency.module.clone(),
                }
            })?;
            if index == target {
                return Err(ProjectError::SelfDependency(module.id.clone()));
            }
            if !targets.insert(target) {
                return Err(ProjectError::DuplicateDependency {
                    module: module.id.clone(),
                    dependency: dependency.module.clone(),
                });
            }
            graph[index].push(target);
            indegree[target] += 1;
        }
        let mut referenced_libraries = HashSet::new();
        for library in &module.libraries {
            if !library_ids.contains_key(library.as_str()) {
                return Err(ProjectError::UnknownReference {
                    kind: "library",
                    id: library.clone(),
                });
            }
            if !referenced_libraries.insert(library) {
                return Err(ProjectError::DuplicateLibraryReference {
                    module: module.id.clone(),
                    library: library.clone(),
                });
            }
        }
        validate_settings(module)?;
    }
    // Kahn's algorithm is iterative and visits each bounded edge once.
    let mut ready: Vec<_> = indegree
        .iter()
        .enumerate()
        .filter_map(|(i, d)| (*d == 0).then_some(i))
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
        return Err(ProjectError::DependencyCycle);
    }
    for module in &mut manifest.modules {
        for root in &mut module.source_roots {
            resolve(&mut root.path, base)?;
        }
    }
    validate_ownership(&manifest.modules)?;
    for library in &mut manifest.libraries {
        for archive in library.binaries.iter_mut().chain(&mut library.sources) {
            resolve(archive, base)?;
        }
    }
    for archive in &mut manifest.sdk_archives {
        resolve(archive, base)?;
    }
    Ok(())
}

fn version<'a>(value: &'a str, field: &'static str) -> Result<(&'a str, &'a str), ProjectError> {
    if let Some((major, minor)) = value.split_once('.')
        && numeric(major)
        && numeric(minor)
    {
        return Ok((major, minor));
    }
    Err(ProjectError::InvalidVersion {
        field,
        value: value.into(),
    })
}

fn numeric(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit())
}
fn numeric_cmp(first: &str, second: &str) -> Ordering {
    let first = first.trim_start_matches('0');
    let second = second.trim_start_matches('0');
    first
        .len()
        .cmp(&second.len())
        .then_with(|| first.cmp(second))
}

fn validate_settings(module: &Module) -> Result<(), ProjectError> {
    let settings = &module.language_settings;
    let language = settings
        .language_version
        .as_deref()
        .map(|v| version(v, "languageVersion"))
        .transpose()?;
    let api = settings
        .api_version
        .as_deref()
        .map(|v| version(v, "apiVersion"))
        .transpose()?;
    if let (Some(language), Some(api)) = (language, api)
        && numeric_cmp(api.0, language.0)
            .then_with(|| numeric_cmp(api.1, language.1))
            .is_gt()
    {
        return Err(ProjectError::ApiVersionExceedsLanguageVersion {
            module: module.id.clone(),
        });
    }
    if let Some(target) = &settings.jvm_target
        && target != "1.8"
        && !(numeric(target) && target.bytes().any(|b| b != b'0'))
    {
        return Err(ProjectError::InvalidVersion {
            field: "jvmTarget",
            value: target.clone(),
        });
    }
    Ok(())
}

fn validate_ownership(modules: &[Module]) -> Result<(), ProjectError> {
    let mut roots: Vec<_> = modules
        .iter()
        .enumerate()
        .flat_map(|(module, m)| m.source_roots.iter().map(move |root| (module, root)))
        .collect();
    roots.sort_unstable_by(|a, b| a.1.path.cmp(&b.1.path));
    // Sorted lexical components keep descendants contiguous. The active stack
    // contains only ancestors; accepted ancestors share module/kind/generated.
    let mut ancestors: Vec<(usize, &SourceRoot)> = Vec::new();
    for (module, root) in roots {
        while ancestors
            .last()
            .is_some_and(|(_, ancestor)| !root.path.starts_with(&ancestor.path))
        {
            ancestors.pop();
        }
        if let Some(&(owner, ancestor)) = ancestors.last()
            && (ancestor.path == root.path
                || owner != module
                || ancestor.kind != root.kind
                || ancestor.generated != root.generated)
        {
            return Err(ProjectError::SourceRootOverlap {
                first_module: modules[owner].id.clone(),
                second_module: modules[module].id.clone(),
                first: ancestor.path.clone(),
                second: root.path.clone(),
            });
        }
        ancestors.push((module, root));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Context {
    Manifest,
    Modules,
    Module,
    Libraries,
    Library,
    Dependencies,
    SourceRoots,
    Archives,
    Other,
}

#[derive(Default)]
struct Counts {
    modules: usize,
    libraries: usize,
    dependencies: usize,
    roots: usize,
    archives: usize,
    exceeded: Option<(Budget, usize)>,
}
impl Counts {
    fn increment(&mut self, context: Context) -> bool {
        let (count, budget, limit) = match context {
            Context::Modules => (&mut self.modules, Budget::Modules, MAX_MODULES),
            Context::Libraries => (&mut self.libraries, Budget::Libraries, MAX_LIBRARIES),
            Context::Dependencies => (
                &mut self.dependencies,
                Budget::Dependencies,
                MAX_DEPENDENCIES,
            ),
            Context::SourceRoots => (&mut self.roots, Budget::SourceRoots, MAX_SOURCE_ROOTS),
            Context::Archives => (&mut self.archives, Budget::ArchivePaths, MAX_ARCHIVE_PATHS),
            _ => return true,
        };
        *count += 1;
        if *count > limit {
            self.exceeded = Some((budget, limit));
            return false;
        }
        true
    }
}

struct Scan<'a> {
    counts: &'a mut Counts,
    context: Context,
}
impl<'de> DeserializeSeed<'de> for Scan<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for Scan<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("JSON project value")
    }
    fn visit_bool<E>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E>(self, _: &str) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<(), A::Error> {
        let child = match self.context {
            Context::Modules => Context::Module,
            Context::Libraries => Context::Library,
            _ => Context::Other,
        };
        // Count before deserializing each item, including the first excess item.
        while sequence
            .next_element_seed(CountedScan {
                counts: self.counts,
                collection: self.context,
                child,
            })?
            .is_some()
        {}
        Ok(())
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some(key) = map.next_key::<String>()? {
            let context = match (self.context, key.as_str()) {
                (Context::Manifest, "modules") => Context::Modules,
                (Context::Manifest, "libraries") => Context::Libraries,
                (Context::Manifest, "sdkArchives") | (Context::Library, "binaries" | "sources") => {
                    Context::Archives
                }
                (Context::Module, "dependencies" | "libraries") => Context::Dependencies,
                (Context::Module, "sourceRoots") => Context::SourceRoots,
                _ => Context::Other,
            };
            map.next_value_seed(Scan {
                counts: self.counts,
                context,
            })?;
        }
        Ok(())
    }
}
struct CountedScan<'a> {
    counts: &'a mut Counts,
    collection: Context,
    child: Context,
}
impl<'de> DeserializeSeed<'de> for CountedScan<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        if !self.counts.increment(self.collection) {
            return Err(serde::de::Error::custom(
                "project collection budget exceeded",
            ));
        }
        Scan {
            counts: self.counts,
            context: self.child,
        }
        .deserialize(deserializer)
    }
}
fn preflight(bytes: &[u8]) -> Result<(), ProjectError> {
    let mut counts = Counts::default();
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let result = Scan {
        counts: &mut counts,
        context: Context::Manifest,
    }
    .deserialize(&mut deserializer);
    if let Some((budget, limit)) = counts.exceeded {
        return Err(ProjectError::BudgetExceeded { budget, limit });
    }
    result.map_err(ProjectError::Json)?;
    deserializer.end().map_err(ProjectError::Json)
}

#[cfg(test)]
mod path_budget_tests {
    use super::*;

    #[test]
    fn arithmetic_overflow_is_a_named_resource_error() {
        for budget in [Budget::CallerPathBytes, Budget::ExpandedPathBytes] {
            assert!(matches!(
                check_path_bytes(usize::MAX.checked_add(1), budget, MAX_EXPANDED_PATH_BYTES),
                Err(ProjectError::BudgetExceeded { budget: actual, limit })
                    if actual == budget && limit == MAX_EXPANDED_PATH_BYTES
            ));
        }
    }
}
