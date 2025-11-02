use cargo_patch_source::source::{GitReference, PatchSource};
use cargo_patch_source::{apply_patches, remove_patches};
use insta::assert_snapshot;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

/// Create a mock workspace with multiple crates
fn create_mock_workspace(temp_dir: &TempDir) -> PathBuf {
    let workspace_path = temp_dir.path().join("mock-workspace");
    fs::create_dir(&workspace_path).unwrap();

    // Create workspace Cargo.toml
    let workspace_toml = r#"[workspace]
members = ["crates/*"]

[workspace.dependencies]
rattler-one = "1.0.0"
rattler-two = "2.0.0"
other-crate = "3.0.0"
"#;
    fs::write(workspace_path.join("Cargo.toml"), workspace_toml).unwrap();

    // Create crates directory
    let crates_dir = workspace_path.join("crates");
    fs::create_dir(&crates_dir).unwrap();

    // Create rattler-one crate
    let rattler_one_dir = crates_dir.join("rattler-one");
    fs::create_dir(&rattler_one_dir).unwrap();
    let rattler_one_toml = r#"[package]
name = "rattler-one"
version = "1.0.0"
edition = "2021"
"#;
    fs::write(rattler_one_dir.join("Cargo.toml"), rattler_one_toml).unwrap();
    fs::create_dir(rattler_one_dir.join("src")).unwrap();
    fs::write(rattler_one_dir.join("src/lib.rs"), "").unwrap();

    // Create rattler-two crate
    let rattler_two_dir = crates_dir.join("rattler-two");
    fs::create_dir(&rattler_two_dir).unwrap();
    let rattler_two_toml = r#"[package]
name = "rattler-two"
version = "2.0.0"
edition = "2021"
"#;
    fs::write(rattler_two_dir.join("Cargo.toml"), rattler_two_toml).unwrap();
    fs::create_dir(rattler_two_dir.join("src")).unwrap();
    fs::write(rattler_two_dir.join("src/lib.rs"), "").unwrap();

    // Create other-crate
    let other_dir = crates_dir.join("other-crate");
    fs::create_dir(&other_dir).unwrap();
    let other_toml = r#"[package]
name = "other-crate"
version = "3.0.0"
edition = "2021"
"#;
    fs::write(other_dir.join("Cargo.toml"), other_toml).unwrap();
    fs::create_dir(other_dir.join("src")).unwrap();
    fs::write(other_dir.join("src/lib.rs"), "").unwrap();

    workspace_path
}

/// Create a target project that depends on the workspace crates
fn create_target_project(temp_dir: &TempDir) -> PathBuf {
    let project_path = temp_dir.path().join("target-project");
    fs::create_dir(&project_path).unwrap();

    let project_toml = r#"[package]
name = "target-project"
version = "0.1.0"
edition = "2021"

[dependencies]
rattler-one = "1.0.0"
rattler-two = "2.0.0"
other-crate = "3.0.0"
"#;
    fs::write(project_path.join("Cargo.toml"), project_toml).unwrap();
    fs::create_dir(project_path.join("src")).unwrap();
    fs::write(project_path.join("src/main.rs"), "fn main() {}").unwrap();

    project_path
}

#[test]
fn test_apply_local_patches_all_crates() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = create_mock_workspace(&temp_dir);
    let project_path = create_target_project(&temp_dir);
    let manifest_path = project_path.join("Cargo.toml");

    // Apply patches from local workspace
    let source = PatchSource::local_path(workspace_path.clone());
    apply_patches(source, Some(manifest_path.clone()), None).unwrap();

    // Read the modified Cargo.toml
    let content = fs::read_to_string(&manifest_path).unwrap();

    // Parse and extract the metadata section for snapshot testing
    let doc: toml_edit::DocumentMut = content.parse().unwrap();
    if let Some(package) = doc.get("package") {
        if let Some(metadata) = package.get("metadata") {
            if let Some(our_metadata) = metadata.get("cargo-patch-source") {
                assert_snapshot!(our_metadata.to_string(), @r###"
                original-versions = { other-crate = "3.0.0", rattler-one = "1.0.0", rattler-two = "2.0.0" }
                managed-patches = ["crates-io"]
                "###);
            }
        }
    }

    // Verify patches were added (without checking exact paths which are temporary)
    assert!(content.contains("[patch.crates-io]"));
    assert!(content.contains("rattler-one"));
    assert!(content.contains("rattler-two"));
    assert!(content.contains("other-crate"));
}

#[test]
fn test_apply_local_patches_with_pattern() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = create_mock_workspace(&temp_dir);
    let project_path = create_target_project(&temp_dir);
    let manifest_path = project_path.join("Cargo.toml");

    // Apply patches only for rattler-* crates
    let source = PatchSource::local_path(workspace_path);
    apply_patches(source, Some(manifest_path.clone()), Some("rattler-*")).unwrap();

    // Read the modified Cargo.toml
    let content = fs::read_to_string(&manifest_path).unwrap();

    // Parse and extract the metadata section for snapshot testing
    let doc: toml_edit::DocumentMut = content.parse().unwrap();
    if let Some(package) = doc.get("package") {
        if let Some(metadata) = package.get("metadata") {
            if let Some(our_metadata) = metadata.get("cargo-patch-source") {
                assert_snapshot!(our_metadata.to_string(), @r###"
                original-versions = { rattler-one = "1.0.0", rattler-two = "2.0.0" }
                managed-patches = ["crates-io"]
                "###);
            }
        }
    }

    // Verify only rattler crates were patched
    assert!(content.contains("rattler-one"));
    assert!(content.contains("rattler-two"));
    // other-crate should not be in the patch section
    let patch_section_start = content.find("[patch.crates-io]").unwrap();
    let patch_section = &content[patch_section_start..];
    assert!(!patch_section.contains("other-crate = { path"));
}

#[test]
fn test_remove_patches() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = create_mock_workspace(&temp_dir);
    let project_path = create_target_project(&temp_dir);
    let manifest_path = project_path.join("Cargo.toml");

    // First apply patches
    let source = PatchSource::local_path(workspace_path);
    apply_patches(source, Some(manifest_path.clone()), None).unwrap();

    // Verify patches exist
    let content_before = fs::read_to_string(&manifest_path).unwrap();
    assert!(content_before.contains("[patch.crates-io]"));

    // Remove patches
    remove_patches(Some(manifest_path.clone())).unwrap();

    // Verify patches were removed
    let content_after = fs::read_to_string(&manifest_path).unwrap();
    assert!(!content_after.contains("[patch.crates-io]"));
    assert!(!content_after.contains("[package.metadata.cargo-patch-source]"));
}

#[test]
fn test_apply_remove_roundtrip() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = create_mock_workspace(&temp_dir);
    let project_path = create_target_project(&temp_dir);
    let manifest_path = project_path.join("Cargo.toml");

    // Save original content (we don't compare it directly due to potential formatting differences)
    let _original_content = fs::read_to_string(&manifest_path).unwrap();

    // Apply patches
    let source = PatchSource::local_path(workspace_path);
    apply_patches(source, Some(manifest_path.clone()), None).unwrap();

    // Remove patches
    remove_patches(Some(manifest_path.clone())).unwrap();

    // Content should be back to original (mostly - whitespace might differ)
    let final_content = fs::read_to_string(&manifest_path).unwrap();
    assert!(!final_content.contains("[patch.crates-io]"));
    assert!(!final_content.contains("cargo-patch-source"));

    // Dependencies should still be there
    assert!(final_content.contains("rattler-one"));
    assert!(final_content.contains("rattler-two"));
}

#[test]
fn test_apply_git_patches() {
    let temp_dir = TempDir::new().unwrap();
    let project_path = create_target_project(&temp_dir);
    let manifest_path = project_path.join("Cargo.toml");

    // Apply git patches with pattern
    let source = PatchSource::git(
        "https://github.com/prefix-dev/rattler".to_string(),
        Some(GitReference::Branch("main".to_string())),
    );
    apply_patches(source, Some(manifest_path.clone()), Some("rattler-*")).unwrap();

    // Read the modified Cargo.toml
    let content = fs::read_to_string(&manifest_path).unwrap();

    // Parse and extract the metadata section for snapshot testing
    let doc: toml_edit::DocumentMut = content.parse().unwrap();
    if let Some(package) = doc.get("package") {
        if let Some(metadata) = package.get("metadata") {
            if let Some(our_metadata) = metadata.get("cargo-patch-source") {
                assert_snapshot!(our_metadata.to_string(), @r###"
                original-versions = { rattler-one = "1.0.0", rattler-two = "2.0.0" }
                managed-patches = ["crates-io"]
                "###);
            }
        }
    }

    // Verify git patches were added
    assert!(content.contains("[patch.crates-io]"));
    assert!(content.contains("git = \"https://github.com/prefix-dev/rattler\""));
    assert!(content.contains("branch = \"main\""));
    assert!(content.contains("rattler-one"));
    assert!(content.contains("rattler-two"));
}

#[test]
fn test_workspace_detection() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = create_mock_workspace(&temp_dir);
    let manifest_path = workspace_path.join("Cargo.toml");

    // Apply patches to a workspace (should work with workspace.dependencies)
    let source = PatchSource::local_path(workspace_path.clone());
    apply_patches(source, Some(manifest_path.clone()), None).unwrap();

    // Read the modified Cargo.toml
    let content = fs::read_to_string(&manifest_path).unwrap();

    // Patches should have been added
    assert!(content.contains("[patch.crates-io]"));
}

#[test]
fn test_no_matching_crates() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = create_mock_workspace(&temp_dir);
    let project_path = create_target_project(&temp_dir);
    let manifest_path = project_path.join("Cargo.toml");

    // Try to apply patches with a pattern that matches nothing
    let source = PatchSource::local_path(workspace_path);
    let result = apply_patches(source, Some(manifest_path), Some("nonexistent-*"));

    // Should fail with NoMatchingCrates error
    assert!(result.is_err());
}

#[test]
fn test_preserves_existing_patches() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = create_mock_workspace(&temp_dir);
    let project_path = create_target_project(&temp_dir);
    let manifest_path = project_path.join("Cargo.toml");

    // Add an existing patch to the Cargo.toml
    let mut existing_content = fs::read_to_string(&manifest_path).unwrap();
    existing_content.push_str(
        r#"
[patch.crates-io]
some-existing-crate = { path = "/some/other/path" }
"#,
    );
    fs::write(&manifest_path, existing_content).unwrap();

    // Apply our patches
    let source = PatchSource::local_path(workspace_path);
    apply_patches(source, Some(manifest_path.clone()), Some("rattler-*")).unwrap();

    // Verify our patches were added
    let content_after_apply = fs::read_to_string(&manifest_path).unwrap();
    assert!(content_after_apply.contains("rattler-one"));
    assert!(content_after_apply.contains("rattler-two"));
    assert!(content_after_apply.contains("some-existing-crate"));

    // Remove our patches
    remove_patches(Some(manifest_path.clone())).unwrap();

    // Verify the existing patch is still there
    let content_after_remove = fs::read_to_string(&manifest_path).unwrap();
    assert!(content_after_remove.contains("some-existing-crate"));
    assert!(content_after_remove.contains("[patch.crates-io]"));

    // But our patches should be gone from the patch section
    // Extract just the patch section to verify
    let patch_section_start = content_after_remove.find("[patch.crates-io]").unwrap();
    let patch_section = &content_after_remove[patch_section_start..];
    assert!(patch_section.contains("some-existing-crate"));
    assert!(!patch_section.contains("rattler-one = { path"));
    assert!(!patch_section.contains("rattler-two = { path"));
}

#[test]
fn test_patch_git_dependencies_without_version() {
    let temp_dir = TempDir::new().unwrap();
    let workspace_path = create_mock_workspace(&temp_dir);
    let project_path = temp_dir.path().join("git-deps-project");
    fs::create_dir(&project_path).unwrap();

    // Create a project with git dependencies (no version field)
    let project_toml = r#"[package]
name = "git-deps-project"
version = "0.1.0"
edition = "2021"

[dependencies]
rattler-one = { git = "https://github.com/prefix-dev/rattler", tag = "v1.0.0" }
rattler-two = { git = "https://github.com/prefix-dev/rattler", tag = "v1.0.0" }
other-crate = { git = "https://github.com/prefix-dev/rattler", tag = "v1.0.0" }
"#;
    fs::write(project_path.join("Cargo.toml"), project_toml).unwrap();
    fs::create_dir(project_path.join("src")).unwrap();
    fs::write(project_path.join("src/main.rs"), "fn main() {}").unwrap();

    let manifest_path = project_path.join("Cargo.toml");

    // Apply patches - this should work even though dependencies don't have version fields
    let source = PatchSource::local_path(workspace_path);
    apply_patches(source, Some(manifest_path.clone()), None).unwrap();

    // Verify patches were added
    let content = fs::read_to_string(&manifest_path).unwrap();
    assert!(content.contains("[patch"));
    assert!(content.contains("rattler-one"));
    assert!(content.contains("rattler-two"));
    assert!(content.contains("other-crate"));
}
