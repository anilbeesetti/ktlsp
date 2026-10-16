
use super::{
    ContentRoot, DependencyScope, ImportError, ImportedLibrary, MAX_BYTES, ModuleDependency,
};
use std::{fmt, io, path::Path};

#[derive(Debug)]
pub struct ImportedWorkspace {
    project: ktlsp_project::v2::ProjectModel,
    pub modules: Vec<ImportedModule>,
    pub libraries: Vec<ImportedLibrary>,
}
impl ImportedWorkspace {
    /// Validated v2 projection; moduleSource placement remains in the sidecar.
    pub fn project_projection(&self) -> &ktlsp_project::v2::ProjectModel {
        &self.project
    }
    /// Canonical catalog shared with the native projection; no mirrored SDK copy.
    pub fn sdks(&self) -> &[ktlsp_project::v2::Sdk] {
        &self.project.sdks
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct ImportedModule {
    pub name: String,
    pub content_roots: Vec<ContentRoot>,
    pub dependencies: Vec<Dependency>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Dependency {
    Module(ModuleDependency),
    Library {
        library: String,
        scope: DependencyScope,
        exported: bool,
    },
    ModuleSource,
    Sdk {
        name: String,
        kind: String,
    },
    InheritedSdk,
}
#[derive(Debug)]
pub enum Error {
    Import(ImportError),
    Project(ktlsp_project::v2::Error),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Import(error) => error.fmt(f),
            Self::Project(error) => write!(f, "incompatible workspace project: {error}"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Import(error) => Some(error),
            Self::Project(error) => Some(error),
        }
    }
}

/// Data-only, unversioned official input; no implicit fallback to the v1 API.
pub fn from_json(
    project_directory: impl AsRef<Path>,
    bytes: &[u8],
) -> Result<ImportedWorkspace, Error> {
    let base = project_directory.as_ref();
    let parsed = super::parse_workspace(base, bytes, true).map_err(Error::Import)?;
    let mut output = Projection {
        bytes: Vec::new(),
        exceeded: false,
    };
    if let Err(error) = serde_json::to_writer(&mut output, &parsed.manifest) {
        return Err(Error::Import(if output.exceeded {
            ImportError::BudgetExceeded("projection bytes")
        } else {
            ImportError::Json(error)
        }));
    }
    let project = ktlsp_project::v2::from_json(base.join("workspace.json"), &output.bytes)
        .map_err(Error::Project)?;
    Ok(ImportedWorkspace {
        project,
        modules: parsed.modules,
        libraries: parsed.libraries,
    })
}

struct Projection {
    bytes: Vec<u8>,
    exceeded: bool,
}
impl io::Write for Projection {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > MAX_BYTES {
            self.exceeded = true;
            return Err(io::Error::other(
                "workspace projection byte budget exceeded",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn parse_sdks(
    workspace: &mut serde_json::Map<String, serde_json::Value>,
    reports: &mut super::Reports,
) -> Result<Vec<ktlsp_project::v2::Sdk>, ImportError> {
    use ktlsp_project::v2::{MAX_SDK_ROOTS, MAX_SDKS, Sdk, SdkRoot};
    let values: Vec<serde_json::Value> = super::default(workspace, "sdks", "", vec![])?;
    if values.len() > MAX_SDKS {
        return Err(ImportError::BudgetExceeded("SDK catalog"));
    }
    let mut sdks = Vec::new();
    let mut root_count = 0usize;
    for (index, value) in values.into_iter().enumerate() {
        let p = format!("/sdks/{index}");
        let mut record = super::object(value, &p)?;
        let name = super::required(&mut record, "name", &p)?;
        let sdk_type = super::required(&mut record, "type", &p)?;
        let version = super::required(&mut record, "version", &p)?;
        let home_path = super::required(&mut record, "homePath", &p)?;
        let additional_data = super::required(&mut record, "additionalData", &p)?;
        let values: Option<Vec<serde_json::Value>> =
            super::default(&mut record, "roots", &p, None)?;
        let roots = if let Some(values) = values {
            root_count = root_count.saturating_add(values.len());
            if root_count > MAX_SDK_ROOTS {
                return Err(ImportError::BudgetExceeded("SDK roots"));
            }
            let mut roots = Vec::new();
            for (index, value) in values.into_iter().enumerate() {
                let p = format!("{p}/roots/{index}");
                let mut record = super::object(value, &p)?;
                roots.push(SdkRoot {
                    url: super::required(&mut record, "url", &p)?,
                    root_type: super::required(&mut record, "type", &p)?,
                });
                reports.unknown(record, &p);
            }
            Some(roots)
        } else {
            None
        };
        sdks.push(Sdk {
            name,
            sdk_type,
            version,
            home_path,
            roots,
            additional_data,
        });
        reports.unknown(record, &p);
    }
    Ok(sdks)
}
