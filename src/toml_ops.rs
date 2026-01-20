use crate::error::{PatchError, Result};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use toml_edit::{DocumentMut, Item, Table};

const METADATA_KEY: &str = "cargo-patch-source";
const ORIGINAL_VERSIONS_KEY: &str = "original-versions";
const MANAGED_PATCHES_KEY: &str = "managed-patches";

/// Read and parse a Cargo.toml file
pub fn read_cargo_toml(path: &Path) -> Result<DocumentMut> {
    let content = fs::read_to_string(path).map_err(|e| PatchError::CargoTomlReadError {
        path: path.to_path_buf(),
        source: e,
    })?;

    content
        .parse::<DocumentMut>()
        .map_err(|e| PatchError::TomlParseError {
            path: path.to_path_buf(),
            source: e,
        })
}

/// Write a Cargo.toml document to file
pub fn write_cargo_toml(path: &Path, doc: &DocumentMut) -> Result<()> {
    fs::write(path, doc.to_string()).map_err(|e| PatchError::CargoTomlWriteError {
        path: path.to_path_buf(),
        source: e,
    })
}

/// Check if the document is a workspace (has `[workspace]` or `[workspace.dependencies]`)
pub fn is_workspace(doc: &DocumentMut) -> bool {
    doc.get("workspace").is_some()
}

/// Get the dependencies table (either workspace.dependencies or dependencies)
pub fn get_dependencies_table_mut(doc: &mut DocumentMut) -> Option<&mut Table> {
    // Check if workspace.dependencies exists first (immutable check)
    let has_workspace_deps = doc
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.as_table())
        .is_some();

    if has_workspace_deps {
        // We know workspace.dependencies exists, so get it mutably
        return doc
            .get_mut("workspace")?
            .get_mut("dependencies")
            .and_then(|d| d.as_table_mut());
    }

    // Fall back to dependencies
    doc.get_mut("dependencies").and_then(|d| d.as_table_mut())
}

/// Get the dependencies table for reading
pub fn get_dependencies_table(doc: &DocumentMut) -> Option<&Table> {
    // Try workspace.dependencies first
    if let Some(workspace) = doc.get("workspace") {
        if let Some(Item::Table(deps)) = workspace.get("dependencies") {
            return Some(deps);
        }
    }

    // Fall back to dependencies
    if let Some(Item::Table(deps)) = doc.get("dependencies") {
        return Some(deps);
    }

    None
}

/// Extract git URL from a dependency specification
pub fn get_dependency_git_url(dep_value: &Item) -> Option<String> {
    match dep_value {
        Item::Value(val) => {
            // Inline table might have git
            if let Some(inline_tbl) = val.as_inline_table() {
                inline_tbl
                    .get("git")
                    .and_then(|g| g.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        }
        Item::Table(table) => {
            // Check for git field in table
            table
                .get("git")
                .and_then(|g| g.as_str())
                .map(|s| s.to_string())
        }
        _ => None,
    }
}

/// Detect if dependencies use a common git URL (returns most common git URL if any)
pub fn detect_common_git_url(doc: &DocumentMut, crate_names: &[String]) -> Option<String> {
    let deps_table = get_dependencies_table(doc)?;

    let mut git_url_counts: HashMap<String, usize> = HashMap::new();

    for crate_name in crate_names {
        if let Some(dep_value) = deps_table.get(crate_name) {
            if let Some(git_url) = get_dependency_git_url(dep_value) {
                *git_url_counts.entry(git_url).or_insert(0) += 1;
            }
        }
    }

    // Return the most common git URL if it accounts for majority of dependencies
    git_url_counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .filter(|(_, count)| *count > crate_names.len() / 2) // Majority rule
        .map(|(url, _)| url)
}

/// Get current version of a dependency
pub fn get_dependency_version(dep_value: &Item) -> Option<String> {
    match dep_value {
        Item::Value(val) => {
            // Simple string version
            if let Some(version) = val.as_str() {
                return Some(version.to_string());
            }
            // Inline table with version field
            if let Some(inline_tbl) = val.as_inline_table() {
                return inline_tbl
                    .get("version")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
            None
        }
        Item::Table(table) => {
            // Table with version field
            table
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        }
        _ => None,
    }
}

/// Update dependency version in the dependencies table
pub fn update_dependency_version(
    doc: &mut DocumentMut,
    crate_name: &str,
    new_version: &str,
) -> Result<()> {
    let deps_table = get_dependencies_table_mut(doc);

    if let Some(deps_table) = deps_table {
        if let Some(dep_value) = deps_table.get_mut(crate_name) {
            match dep_value {
                Item::Value(val) => {
                    // Simple string version - replace the entire item
                    if val.is_str() {
                        *dep_value = toml_edit::value(new_version);
                    }
                    // Inline table - update the version field
                    else if let Some(inline_tbl) = val.as_inline_table_mut() {
                        if inline_tbl.contains_key("version") {
                            inline_tbl.insert("version", new_version.into());
                        }
                    }
                }
                Item::Table(table) => {
                    // Table with version field - update it
                    if table.contains_key("version") {
                        table.insert("version", toml_edit::value(new_version));
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

/// Get or create the metadata table for cargo-patch-source
fn get_or_create_metadata_table(doc: &mut DocumentMut) -> &mut Table {
    // Check if workspace or package exists
    let is_workspace = doc.get("workspace").is_some();

    let metadata_path = if is_workspace {
        vec!["workspace", "metadata", METADATA_KEY]
    } else {
        vec!["package", "metadata", METADATA_KEY]
    };

    // Navigate/create the nested structure
    let mut current = doc.as_table_mut();
    for key in metadata_path {
        current = current
            .entry(key)
            .or_insert(Item::Table(Table::new()))
            .as_table_mut()
            .unwrap();
    }

    current
}

/// Get the metadata table for reading (returns None if doesn't exist)
fn get_metadata_table(doc: &DocumentMut) -> Option<&Table> {
    // Try workspace first
    if let Some(workspace) = doc.get("workspace") {
        if let Some(metadata) = workspace.get("metadata") {
            if let Some(Item::Table(our_metadata)) = metadata.get(METADATA_KEY) {
                return Some(our_metadata);
            }
        }
    }

    // Try package
    if let Some(package) = doc.get("package") {
        if let Some(metadata) = package.get("metadata") {
            if let Some(Item::Table(our_metadata)) = metadata.get(METADATA_KEY) {
                return Some(our_metadata);
            }
        }
    }

    None
}

/// Store original versions in metadata
pub fn store_original_versions(
    doc: &mut DocumentMut,
    versions: &HashMap<String, String>,
) -> Result<()> {
    let metadata = get_or_create_metadata_table(doc);

    // Create a table for versions with sorted keys for deterministic output
    let mut versions_table = toml_edit::InlineTable::new();

    // Sort keys for deterministic ordering
    let mut sorted_versions: Vec<_> = versions.iter().collect();
    sorted_versions.sort_by_key(|(name, _)| *name);

    for (name, version) in sorted_versions {
        versions_table.insert(name, version.as_str().into());
    }

    metadata.insert(
        ORIGINAL_VERSIONS_KEY,
        Item::Value(toml_edit::Value::InlineTable(versions_table)),
    );

    Ok(())
}

/// Get original versions from metadata
pub fn get_original_versions(doc: &DocumentMut) -> Result<HashMap<String, String>> {
    let Some(metadata) = get_metadata_table(doc) else {
        return Ok(HashMap::new());
    };

    let Some(versions_item) = metadata.get(ORIGINAL_VERSIONS_KEY) else {
        return Ok(HashMap::new());
    };

    let mut result = HashMap::new();

    // Handle both inline table and regular table
    match versions_item {
        Item::Value(val) => {
            if let Some(inline_table) = val.as_inline_table() {
                for (key, value) in inline_table.iter() {
                    if let Some(version_str) = value.as_str() {
                        result.insert(key.to_string(), version_str.to_string());
                    }
                }
            }
        }
        Item::Table(table) => {
            for (key, value) in table.iter() {
                if let Some(version_str) = value.as_str() {
                    result.insert(key.to_string(), version_str.to_string());
                }
            }
        }
        _ => {}
    }

    Ok(result)
}

/// Add a patch source to the managed list
pub fn add_managed_patch(doc: &mut DocumentMut, patch_key: &str) -> Result<()> {
    let metadata = get_or_create_metadata_table(doc);

    // Get existing managed patches or create new array
    let managed =
        metadata
            .entry(MANAGED_PATCHES_KEY)
            .or_insert(Item::Value(
                toml_edit::Value::Array(toml_edit::Array::new()),
            ));

    if let Some(array) = managed.as_array_mut() {
        // Add if not already present
        let patch_key_val =
            toml_edit::Value::String(toml_edit::Formatted::new(patch_key.to_string()));
        if !array.iter().any(|v| v.as_str() == Some(patch_key)) {
            array.push(patch_key_val);
        }
    }

    Ok(())
}

/// Get list of managed patch sources
pub fn get_managed_patches(doc: &DocumentMut) -> Vec<String> {
    let Some(metadata) = get_metadata_table(doc) else {
        return Vec::new();
    };

    let Some(managed_item) = metadata.get(MANAGED_PATCHES_KEY) else {
        return Vec::new();
    };

    let Some(array) = managed_item.as_array() else {
        return Vec::new();
    };

    array
        .iter()
        .filter_map(|v| v.as_str().map(|s| s.to_string()))
        .collect()
}

/// Add or update a patch section
pub fn add_patch_section(
    doc: &mut DocumentMut,
    patch_key: &str,
    crate_name: &str,
    patch_spec: Table,
) {
    // Get or create the patch table
    let patch_table = doc
        .entry("patch")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .unwrap();

    // Get or create the specific patch source table (e.g., patch.crates-io)
    let source_table = patch_table
        .entry(patch_key)
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .unwrap();

    // Add the crate patch
    source_table.insert(crate_name, Item::Table(patch_spec));
}

/// Remove all managed patch sections (using metadata tracking)
pub fn remove_managed_patches(doc: &mut DocumentMut) -> Result<bool> {
    // Get list of managed patches from metadata
    let managed_patches = get_managed_patches(doc);

    if managed_patches.is_empty() {
        return Err(PatchError::NoPatchesFound);
    }

    // Get the crates we patched from original-versions
    let original_versions = get_original_versions(doc)?;
    let patched_crates: Vec<String> = original_versions.keys().cloned().collect();

    let Some(patch_table) = doc.get_mut("patch").and_then(|p| p.as_table_mut()) else {
        return Err(PatchError::NoPatchesFound);
    };

    // For each managed patch key, remove only the specific crates we added
    for patch_key in &managed_patches {
        if let Some(source_table) = patch_table
            .get_mut(patch_key)
            .and_then(|t| t.as_table_mut())
        {
            // Remove each crate patch we added
            for crate_name in &patched_crates {
                source_table.remove(crate_name);
            }

            // If the source table is now empty, remove it entirely
            if source_table.is_empty() {
                patch_table.remove(patch_key);
            }
        }
    }

    // If patch table is empty, remove it entirely
    if patch_table.is_empty() {
        doc.remove("patch");
    }

    // Clear metadata
    clear_metadata(doc)?;

    Ok(true)
}

/// Clear all cargo-patch-source metadata
fn clear_metadata(doc: &mut DocumentMut) -> Result<()> {
    // Try workspace first
    if let Some(workspace) = doc.get_mut("workspace") {
        if let Some(metadata) = workspace.get_mut("metadata") {
            if let Some(metadata_table) = metadata.as_table_mut() {
                metadata_table.remove(METADATA_KEY);

                // Clean up empty metadata table
                if metadata_table.is_empty() {
                    if let Some(workspace_table) = workspace.as_table_mut() {
                        workspace_table.remove("metadata");
                    }
                }
            }
        }
    }

    // Try package
    if let Some(package) = doc.get_mut("package") {
        if let Some(metadata) = package.get_mut("metadata") {
            if let Some(metadata_table) = metadata.as_table_mut() {
                metadata_table.remove(METADATA_KEY);

                // Clean up empty metadata table
                if metadata_table.is_empty() {
                    if let Some(package_table) = package.as_table_mut() {
                        package_table.remove("metadata");
                    }
                }
            }
        }
    }

    Ok(())
}
