use crate::cargo_ops::{filter_crates_by_pattern, query_workspace_crates};
use crate::error::{PatchError, Result};
use crate::source::{GitReference, PatchSource, SourceWorkspacePath, TargetManifestPath};
use crate::toml_ops::{
    add_managed_patch, detect_common_git_url, get_dependencies_table, get_dependency_version,
    get_original_versions, read_cargo_toml, remove_managed_patches, store_original_versions,
    update_dependency_version, write_cargo_toml,
};
use std::collections::HashMap;
use std::path::PathBuf;
use toml_edit::Table;

/// Apply patches from a source to a target Cargo.toml
pub fn apply_patches(
    source: PatchSource,
    target_manifest_path: Option<PathBuf>,
    pattern: Option<&str>,
) -> Result<()> {
    // Determine the target manifest path (defaults to ./Cargo.toml)
    let target_manifest_path = TargetManifestPath::new(
        target_manifest_path.unwrap_or_else(|| std::env::current_dir().unwrap().join("Cargo.toml")),
    );

    if !target_manifest_path.as_path().exists() {
        return Err(PatchError::SourceNotFound {
            path: target_manifest_path.as_path().to_path_buf(),
        });
    }

    // Read the target Cargo.toml (the manifest we're going to patch)
    let mut target_doc = read_cargo_toml(target_manifest_path.as_path())?;

    // Get current dependencies from the target to know which crates to patch
    let current_deps = get_dependencies_table(&target_doc)
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
        PatchSource::LocalPath(source_workspace_path) => {
            apply_local_path_patches(
                &mut target_doc,
                &source_workspace_path,
                &current_deps,
                pattern,
            )?;
        }
        PatchSource::Git { url, reference } => {
            apply_git_patches(&mut target_doc, &url, reference, &current_deps, pattern)?;
        }
    }

    // Write back the modified target Cargo.toml
    write_cargo_toml(target_manifest_path.as_path(), &target_doc)?;

    println!(
        "Successfully applied patches to {}",
        target_manifest_path.as_path().display()
    );
    Ok(())
}

/// Apply patches from a local source workspace to the target manifest
fn apply_local_path_patches(
    target_doc: &mut toml_edit::DocumentMut,
    source_workspace_path: &SourceWorkspacePath,
    current_deps: &HashMap<String, String>,
    pattern: Option<&str>,
) -> Result<()> {
    // Query the source workspace for available crates
    let source_workspace_crates = query_workspace_crates(source_workspace_path.as_path())?;

    // Filter by pattern if provided
    let source_workspace_crates = filter_crates_by_pattern(source_workspace_crates, pattern)?;

    // Filter to only crates that are in current target dependencies
    let crates_to_patch: Vec<_> = source_workspace_crates
        .into_iter()
        .filter(|c| current_deps.contains_key(&c.name))
        .collect();

    if crates_to_patch.is_empty() {
        println!("No matching crates found in current dependencies");
        return Ok(());
    }

    // Collect crate names for git URL detection in the target
    let crate_names: Vec<String> = crates_to_patch.iter().map(|c| c.name.clone()).collect();

    // Detect if these dependencies in the target come from a common git URL
    let git_url = detect_common_git_url(target_doc, &crate_names);

    // Store original versions from target dependencies table (not our stored versions)
    let mut original_versions = HashMap::new();
    if let Some(deps_table) = get_dependencies_table(target_doc) {
        for crate_name in &crate_names {
            if let Some(dep_value) = deps_table.get(crate_name) {
                if let Some(version) = get_dependency_version(dep_value) {
                    original_versions.insert(crate_name.clone(), version);
                }
            }
        }
    }

    // Update versions in target [workspace.dependencies] to match source local versions
    for crate_info in &crates_to_patch {
        update_dependency_version(target_doc, &crate_info.name, &crate_info.version)?;
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

    // Store original versions and track managed patch in target metadata
    store_original_versions(target_doc, &original_versions)?;
    add_managed_patch(target_doc, patch_key)?;

    // Add patch section to target document
    target_doc
        .entry("patch")
        .or_insert(toml_edit::Item::Table(Table::new()))
        .as_table_mut()
        .unwrap()
        .insert(patch_key, toml_edit::Item::Table(patch_table));

    Ok(())
}

/// Apply patches from a git repository to the target manifest
fn apply_git_patches(
    target_doc: &mut toml_edit::DocumentMut,
    git_url: &str,
    reference: Option<GitReference>,
    current_deps: &HashMap<String, String>,
    pattern: Option<&str>,
) -> Result<()> {
    // For git patches, we can't easily query the remote repository
    // So we'll patch all target dependencies that match the pattern (or all if no pattern)

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

    // Store original versions and track managed patch in target metadata
    store_original_versions(target_doc, &original_versions)?;
    add_managed_patch(target_doc, "crates-io")?;

    // Add patch section to target document under [patch.crates-io]
    target_doc
        .entry("patch")
        .or_insert(toml_edit::Item::Table(Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("crates-io", toml_edit::Item::Table(patch_table));

    Ok(())
}

/// Remove patches from a target Cargo.toml
pub fn remove_patches(target_manifest_path: Option<PathBuf>, pattern: Option<&str>) -> Result<()> {
    // Determine the target manifest path (defaults to ./Cargo.toml)
    let target_manifest_path = TargetManifestPath::new(
        target_manifest_path.unwrap_or_else(|| std::env::current_dir().unwrap().join("Cargo.toml")),
    );

    if !target_manifest_path.as_path().exists() {
        return Err(PatchError::SourceNotFound {
            path: target_manifest_path.as_path().to_path_buf(),
        });
    }

    // Read the target Cargo.toml (the manifest we're going to modify)
    let mut target_doc = read_cargo_toml(target_manifest_path.as_path())?;

    // If pattern is specified, we need to selectively remove patches
    // For now, we'll remove all managed patches
    // TODO: Implement pattern-based removal
    if pattern.is_some() {
        println!(
            "Warning: Pattern-based removal not yet implemented, removing all managed patches"
        );
    }

    // Get original versions from target metadata
    let original_versions = get_original_versions(&target_doc)?;

    // Restore original versions in target before removing patches
    if !original_versions.is_empty() {
        println!(
            "Restoring original versions for {} crates",
            original_versions.len()
        );
        for (crate_name, version) in &original_versions {
            update_dependency_version(&mut target_doc, crate_name, version)?;
        }
    }

    // Remove all managed patches from target
    let removed = remove_managed_patches(&mut target_doc)?;

    if removed {
        // Write back the modified target Cargo.toml
        write_cargo_toml(target_manifest_path.as_path(), &target_doc)?;
        println!(
            "Successfully removed patches from {}",
            target_manifest_path.as_path().display()
        );
        Ok(())
    } else {
        Err(PatchError::NoPatchesFound)
    }
}
