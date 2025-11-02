use crate::error::{PatchError, Result};
use cargo_metadata::MetadataCommand;
use std::collections::HashMap;
use std::path::Path;

/// Information about a crate that can be patched
#[derive(Debug, Clone)]
pub struct CrateInfo {
    pub name: String,
    pub version: String,
    pub manifest_path: std::path::PathBuf,
}

/// Query metadata for a workspace at the given path
pub fn query_workspace_crates(workspace_path: &Path) -> Result<Vec<CrateInfo>> {
    let manifest_path = workspace_path.join("Cargo.toml");

    if !manifest_path.exists() {
        return Err(PatchError::SourceNotFound {
            path: manifest_path,
        });
    }

    let metadata = MetadataCommand::new()
        .manifest_path(&manifest_path)
        .exec()
        .map_err(|e| PatchError::CargoMetadataError { source: e })?;

    let workspace_members: Vec<_> = metadata
        .workspace_packages()
        .into_iter()
        .map(|pkg| CrateInfo {
            name: pkg.name.clone(),
            version: pkg.version.to_string(),
            manifest_path: pkg.manifest_path.clone().into_std_path_buf(),
        })
        .collect();

    if workspace_members.is_empty() {
        return Err(PatchError::NotAWorkspace {
            path: workspace_path.to_path_buf(),
        });
    }

    Ok(workspace_members)
}

/// Query metadata for the current/target Cargo.toml to find which dependencies are used
pub fn query_current_dependencies(manifest_path: &Path) -> Result<HashMap<String, String>> {
    let metadata = MetadataCommand::new()
        .manifest_path(manifest_path)
        .exec()
        .map_err(|e| PatchError::CargoMetadataError { source: e })?;

    let mut dependencies = HashMap::new();

    // Collect workspace dependencies if this is a workspace root
    if let Some(resolve) = metadata.resolve {
        for node in resolve.nodes {
            if let Some(pkg) = metadata.packages.iter().find(|p| p.id == node.id) {
                dependencies.insert(pkg.name.clone(), pkg.version.to_string());
            }
        }
    }

    Ok(dependencies)
}

/// Filter crates by pattern (supports wildcards)
pub fn filter_crates_by_pattern(
    crates: Vec<CrateInfo>,
    pattern: Option<&str>,
) -> Result<Vec<CrateInfo>> {
    let Some(pattern) = pattern else {
        return Ok(crates);
    };

    // Convert glob pattern to regex
    let regex_pattern = pattern
        .replace(".", r"\.")
        .replace("*", ".*")
        .replace("?", ".");
    let regex_pattern = format!("^{}$", regex_pattern);

    let re = regex::Regex::new(&regex_pattern).map_err(|e| PatchError::InvalidPattern {
        pattern: pattern.to_string(),
        source: e,
    })?;

    let filtered: Vec<_> = crates
        .into_iter()
        .filter(|c| re.is_match(&c.name))
        .collect();

    if filtered.is_empty() {
        return Err(PatchError::NoMatchingCrates {
            pattern: pattern.to_string(),
        });
    }

    Ok(filtered)
}
