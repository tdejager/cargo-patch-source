use crate::cargo_ops::{filter_crates_by_pattern, query_workspace_crates};
use crate::error::{PatchError, Result};
use crate::source::{GitReference, PatchSource};
use crate::toml_ops::{
    add_managed_patch, detect_common_git_url, get_dependencies_table, get_dependency_version,
    get_original_versions, read_cargo_toml, remove_managed_patches, store_original_versions,
    update_dependency_version, write_cargo_toml,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use toml_edit::Table;

/// Apply patches from a source to a Cargo.toml
pub fn apply_patches(
    source: PatchSource,
    manifest_path: Option<PathBuf>,
    pattern: Option<&str>,
) -> Result<()> {
    // Determine the manifest path
    let manifest_path =
        manifest_path.unwrap_or_else(|| std::env::current_dir().unwrap().join("Cargo.toml"));

    if !manifest_path.exists() {
        return Err(PatchError::SourceNotFound {
            path: manifest_path,
        });
    }

    // Read the target Cargo.toml
    let mut doc = read_cargo_toml(&manifest_path)?;

    // Get current dependencies to know which crates to patch
    let current_deps = get_dependencies_table(&doc)
        .map(|t| {
            t.iter()
                .filter_map(|(k, v)| {
                    // Extract version if it exists
                    match v {
                        toml_edit::Item::Value(val) => {
                            // Handle simple string version
                            if let Some(version) = val.as_str() {
                                Some((k.to_string(), version.to_string()))
                            }
                            // Handle inline table: { version = "...", ... }
                            else if let Some(inline_tbl) = val.as_inline_table() {
                                inline_tbl
                                    .get("version")
                                    .and_then(|v| v.as_str())
                                    .map(|version| (k.to_string(), version.to_string()))
                            } else {
                                None
                            }
                        }
                        toml_edit::Item::Table(tbl) => tbl
                            .get("version")
                            .and_then(|v| v.as_str())
                            .map(|version| (k.to_string(), version.to_string())),
                        _ => None,
                    }
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();

    match source {
        PatchSource::LocalPath(path) => {
            apply_local_path_patches(&mut doc, &path, &current_deps, pattern)?;
        }
        PatchSource::Git { url, reference } => {
            apply_git_patches(&mut doc, &url, reference, &current_deps, pattern)?;
        }
    }

    // Write back the modified Cargo.toml
    write_cargo_toml(&manifest_path, &doc)?;

    println!(
        "Successfully applied patches to {}",
        manifest_path.display()
    );
    Ok(())
}

/// Apply patches from a local workspace path
fn apply_local_path_patches(
    doc: &mut toml_edit::DocumentMut,
    workspace_path: &Path,
    current_deps: &HashMap<String, String>,
    pattern: Option<&str>,
) -> Result<()> {
    // Query the workspace for crates
    let workspace_crates = query_workspace_crates(workspace_path)?;

    // Filter by pattern if provided
    let workspace_crates = filter_crates_by_pattern(workspace_crates, pattern)?;

    // Filter to only crates that are in current dependencies
    let crates_to_patch: Vec<_> = workspace_crates
        .into_iter()
        .filter(|c| current_deps.contains_key(&c.name))
        .collect();

    if crates_to_patch.is_empty() {
        println!("No matching crates found in current dependencies");
        return Ok(());
    }

    // Collect crate names for git URL detection
    let crate_names: Vec<String> = crates_to_patch.iter().map(|c| c.name.clone()).collect();

    // Detect if these dependencies come from a common git URL
    let git_url = detect_common_git_url(doc, &crate_names);

    // Store original versions from dependencies table (not our stored versions)
    let mut original_versions = HashMap::new();
    if let Some(deps_table) = get_dependencies_table(doc) {
        for crate_name in &crate_names {
            if let Some(dep_value) = deps_table.get(crate_name) {
                if let Some(version) = get_dependency_version(dep_value) {
                    original_versions.insert(crate_name.clone(), version);
                }
            }
        }
    }

    // Update versions in [workspace.dependencies] to match local versions
    for crate_info in &crates_to_patch {
        update_dependency_version(doc, &crate_info.name, &crate_info.version)?;
    }

    // Create patch entries
    let mut patch_table = Table::new();
    for crate_info in &crates_to_patch {
        let mut crate_patch = toml_edit::InlineTable::new();

        // Get the path to the crate (directory containing its Cargo.toml)
        let crate_path = crate_info
            .manifest_path
            .parent()
            .expect("Crate manifest should have a parent directory");

        crate_patch.insert("path", crate_path.display().to_string().into());

        patch_table.insert(
            &crate_info.name,
            toml_edit::Item::Value(toml_edit::Value::InlineTable(crate_patch)),
        );

        println!(
            "  Patching {} {} -> {}",
            crate_info.name,
            crate_info.version,
            crate_path.display()
        );
    }

    // Determine patch key (crates-io or git URL)
    let patch_key = if let Some(url) = git_url.as_ref() {
        println!("  Detected git source: {}", url);
        url.as_str()
    } else {
        "crates-io"
    };

    // Store original versions and track managed patch in metadata
    store_original_versions(doc, &original_versions)?;
    add_managed_patch(doc, patch_key)?;

    // Add to document under appropriate patch section
    doc.entry("patch")
        .or_insert(toml_edit::Item::Table(Table::new()))
        .as_table_mut()
        .unwrap()
        .insert(patch_key, toml_edit::Item::Table(patch_table));

    Ok(())
}

/// Apply patches from a git repository
fn apply_git_patches(
    doc: &mut toml_edit::DocumentMut,
    git_url: &str,
    reference: Option<GitReference>,
    current_deps: &HashMap<String, String>,
    pattern: Option<&str>,
) -> Result<()> {
    // For git patches, we can't easily query the remote repository
    // So we'll patch all dependencies that match the pattern (or all if no pattern)

    let crates_to_patch: Vec<_> = if let Some(pattern) = pattern {
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

        current_deps
            .keys()
            .filter(|name| re.is_match(name))
            .cloned()
            .collect()
    } else {
        // If no pattern, we need user to specify which crates
        // For now, return error - user should use pattern with git
        return Err(PatchError::NoMatchingCrates {
            pattern: "none specified (pattern required for git sources)".to_string(),
        });
    };

    if crates_to_patch.is_empty() {
        return Err(PatchError::NoMatchingCrates {
            pattern: pattern.unwrap_or("none").to_string(),
        });
    }

    // Store original versions
    let mut original_versions = HashMap::new();
    for crate_name in &crates_to_patch {
        if let Some(version) = current_deps.get(crate_name) {
            original_versions.insert(crate_name.clone(), version.clone());
        }
    }

    // Create patch entries
    let mut patch_table = Table::new();
    for crate_name in &crates_to_patch {
        let mut crate_patch = toml_edit::InlineTable::new();

        crate_patch.insert("git", git_url.into());

        // Add reference if specified
        match &reference {
            Some(GitReference::Branch(b)) => {
                crate_patch.insert("branch", b.as_str().into());
            }
            Some(GitReference::Tag(t)) => {
                crate_patch.insert("tag", t.as_str().into());
            }
            Some(GitReference::Rev(r)) => {
                crate_patch.insert("rev", r.as_str().into());
            }
            None => {}
        }

        patch_table.insert(
            crate_name,
            toml_edit::Item::Value(toml_edit::Value::InlineTable(crate_patch)),
        );

        let ref_str = match &reference {
            Some(GitReference::Branch(b)) => format!(" (branch: {})", b),
            Some(GitReference::Tag(t)) => format!(" (tag: {})", t),
            Some(GitReference::Rev(r)) => format!(" (rev: {})", r),
            None => String::new(),
        };

        println!("  Patching {} -> {}{}", crate_name, git_url, ref_str);
    }

    // Store original versions and track managed patch in metadata
    store_original_versions(doc, &original_versions)?;
    add_managed_patch(doc, "crates-io")?;

    // Add to document under [patch.crates-io]
    doc.entry("patch")
        .or_insert(toml_edit::Item::Table(Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("crates-io", toml_edit::Item::Table(patch_table));

    Ok(())
}

/// Remove patches from a Cargo.toml
pub fn remove_patches(manifest_path: Option<PathBuf>, pattern: Option<&str>) -> Result<()> {
    // Determine the manifest path
    let manifest_path =
        manifest_path.unwrap_or_else(|| std::env::current_dir().unwrap().join("Cargo.toml"));

    if !manifest_path.exists() {
        return Err(PatchError::SourceNotFound {
            path: manifest_path,
        });
    }

    // Read the target Cargo.toml
    let mut doc = read_cargo_toml(&manifest_path)?;

    // If pattern is specified, we need to selectively remove patches
    // For now, we'll remove all managed patches
    // TODO: Implement pattern-based removal
    if pattern.is_some() {
        println!(
            "Warning: Pattern-based removal not yet implemented, removing all managed patches"
        );
    }

    // Get original versions from metadata
    let original_versions = get_original_versions(&doc)?;

    // Restore original versions before removing patches
    if !original_versions.is_empty() {
        println!(
            "Restoring original versions for {} crates",
            original_versions.len()
        );
        for (crate_name, version) in &original_versions {
            update_dependency_version(&mut doc, crate_name, version)?;
        }
    }

    // Remove all managed patches
    let removed = remove_managed_patches(&mut doc)?;

    if removed {
        // Write back the modified Cargo.toml
        write_cargo_toml(&manifest_path, &doc)?;
        println!(
            "Successfully removed patches from {}",
            manifest_path.display()
        );
        Ok(())
    } else {
        Err(PatchError::NoPatchesFound)
    }
}
