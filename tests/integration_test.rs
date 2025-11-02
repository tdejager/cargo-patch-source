use cargo_patch_source::source::{GitReference, PatchSource};
use cargo_patch_source::{apply_patches, remove_patches};
use insta::assert_snapshot;
use toml_edit::DocumentMut;

mod support;

use support::{DependencySpec, Project, TestFixture, Workspace};

fn rattler_workspace(fixture: &TestFixture) -> Workspace {
    fixture
        .workspace("mock-workspace")
        .member("rattler-one", "1.0.0")
        .member("rattler-two", "2.0.0")
        .member("other-crate", "3.0.0")
        .build()
}

fn rattler_project(fixture: &TestFixture) -> Project {
    fixture
        .project("target-project")
        .dep_version("rattler-one", "1.0.0")
        .dep_version("rattler-two", "2.0.0")
        .dep_version("other-crate", "3.0.0")
        .build()
}

fn normalize_manifest(content: &str, workspace: Option<&Workspace>) -> String {
    let mut normalized = content.to_string();
    if let Some(ws) = workspace {
        if let Some(ws_str) = ws.path().to_str() {
            normalized = normalized.replace(ws_str, "<workspace>");
        }
    }
    normalized
}

#[test]
fn test_apply_local_patches_all_crates() {
    let fixture = TestFixture::new();
    let workspace = rattler_workspace(&fixture);
    let project = rattler_project(&fixture);
    let manifest_path = project.manifest_path().to_path_buf();

    apply_patches(
        PatchSource::local_path(workspace.path().to_path_buf()),
        Some(manifest_path.clone()),
        None,
    )
    .unwrap();

    let content = project.read_manifest();
    let normalized = normalize_manifest(&content, Some(&workspace));
    let doc: DocumentMut = content.parse().unwrap();
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

    assert_snapshot!(
        normalized.as_str(),
        @r###"
[package]
name = "target-project"
version = "0.1.0"
edition = "2021"

[package.metadata]

[package.metadata.cargo-patch-source]
original-versions = { other-crate = "3.0.0", rattler-one = "1.0.0", rattler-two = "2.0.0" }
managed-patches = ["crates-io"]

[dependencies]
other-crate = "3.0.0"
rattler-one = "1.0.0"
rattler-two = "2.0.0"

[patch]

[patch.crates-io]
other-crate = { path = "<workspace>/crates/other-crate" }
rattler-one = { path = "<workspace>/crates/rattler-one" }
rattler-two = { path = "<workspace>/crates/rattler-two" }
"###
    );
}

#[test]
fn test_apply_local_patches_with_pattern() {
    let fixture = TestFixture::new();
    let workspace = rattler_workspace(&fixture);
    let project = rattler_project(&fixture);
    let manifest_path = project.manifest_path().to_path_buf();

    apply_patches(
        PatchSource::local_path(workspace.path().to_path_buf()),
        Some(manifest_path.clone()),
        Some("rattler-*"),
    )
    .unwrap();

    let content = project.read_manifest();
    let normalized = normalize_manifest(&content, Some(&workspace));
    let doc: DocumentMut = content.parse().unwrap();
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

    assert_snapshot!(
        normalized.as_str(),
        @r###"
[package]
name = "target-project"
version = "0.1.0"
edition = "2021"

[package.metadata]

[package.metadata.cargo-patch-source]
original-versions = { rattler-one = "1.0.0", rattler-two = "2.0.0" }
managed-patches = ["crates-io"]

[dependencies]
other-crate = "3.0.0"
rattler-one = "1.0.0"
rattler-two = "2.0.0"

[patch]

[patch.crates-io]
rattler-one = { path = "<workspace>/crates/rattler-one" }
rattler-two = { path = "<workspace>/crates/rattler-two" }
"###
    );

    let patch_table = doc
        .get("patch")
        .and_then(|p| p.get("crates-io"))
        .and_then(|item| item.as_table())
        .cloned()
        .unwrap();

    let mut patched_crates: Vec<_> = patch_table.iter().map(|(k, _)| k.to_string()).collect();
    patched_crates.sort();
    let patched_crates_repr = format!("{:?}", patched_crates);
    assert_snapshot!(
        patched_crates_repr.as_str(),
        @r###"["rattler-one", "rattler-two"]"###
    );
}

#[test]
fn test_remove_patches() {
    let fixture = TestFixture::new();
    let workspace = rattler_workspace(&fixture);
    let project = rattler_project(&fixture);
    let manifest_path = project.manifest_path().to_path_buf();

    apply_patches(
        PatchSource::local_path(workspace.path().to_path_buf()),
        Some(manifest_path.clone()),
        None,
    )
    .unwrap();

    let content_before = project.read_manifest();
    let normalized_before = normalize_manifest(&content_before, Some(&workspace));
    assert_snapshot!(
        normalized_before.as_str(),
        @r###"
[package]
name = "target-project"
version = "0.1.0"
edition = "2021"

[package.metadata]

[package.metadata.cargo-patch-source]
original-versions = { other-crate = "3.0.0", rattler-one = "1.0.0", rattler-two = "2.0.0" }
managed-patches = ["crates-io"]

[dependencies]
other-crate = "3.0.0"
rattler-one = "1.0.0"
rattler-two = "2.0.0"

[patch]

[patch.crates-io]
other-crate = { path = "<workspace>/crates/other-crate" }
rattler-one = { path = "<workspace>/crates/rattler-one" }
rattler-two = { path = "<workspace>/crates/rattler-two" }
"###
    );

    remove_patches(Some(manifest_path.clone())).unwrap();

    let content_after = project.read_manifest();
    let normalized_after = normalize_manifest(&content_after, Some(&workspace));
    assert_snapshot!(
        normalized_after.as_str(),
        @r###"
[package]
name = "target-project"
version = "0.1.0"
edition = "2021"

[dependencies]
other-crate = "3.0.0"
rattler-one = "1.0.0"
rattler-two = "2.0.0"
"###
    );
}

#[test]
fn test_apply_remove_roundtrip() {
    let fixture = TestFixture::new();
    let workspace = rattler_workspace(&fixture);
    let project = rattler_project(&fixture);
    let manifest_path = project.manifest_path().to_path_buf();

    let _original_content = project.read_manifest();

    apply_patches(
        PatchSource::local_path(workspace.path().to_path_buf()),
        Some(manifest_path.clone()),
        None,
    )
    .unwrap();

    remove_patches(Some(manifest_path.clone())).unwrap();

    let final_content = project.read_manifest();
    let normalized = normalize_manifest(&final_content, Some(&workspace));
    assert_snapshot!(
        normalized.as_str(),
        @r###"
[package]
name = "target-project"
version = "0.1.0"
edition = "2021"

[dependencies]
other-crate = "3.0.0"
rattler-one = "1.0.0"
rattler-two = "2.0.0"
"###
    );
}

#[test]
fn test_apply_git_patches() {
    let fixture = TestFixture::new();
    let project = rattler_project(&fixture);
    let manifest_path = project.manifest_path().to_path_buf();

    let source = PatchSource::git(
        "https://github.com/prefix-dev/rattler".to_string(),
        Some(GitReference::Branch("main".to_string())),
    );
    apply_patches(source, Some(manifest_path.clone()), Some("rattler-*")).unwrap();

    let content = project.read_manifest();
    let doc: DocumentMut = content.parse().unwrap();
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

    let patch_crates_io = doc
        .get("patch")
        .and_then(|p| p.get("crates-io"))
        .and_then(|item| item.as_table())
        .cloned()
        .unwrap();

    let mut entries: Vec<_> = patch_crates_io
        .iter()
        .map(|(name, value)| {
            let value_str = value.to_string();
            format!("{} = {}", name, value_str.trim_start())
        })
        .collect();
    entries.sort();
    let patch_snapshot = entries.join("\n");

    assert_snapshot!(
        patch_snapshot.as_str(),
        @r###"
rattler-one = { git = "https://github.com/prefix-dev/rattler", branch = "main" }
rattler-two = { git = "https://github.com/prefix-dev/rattler", branch = "main" }
"###
    );
}

#[test]
fn test_workspace_detection() {
    let fixture = TestFixture::new();
    let workspace = rattler_workspace(&fixture);
    let manifest_path = workspace.manifest_path().to_path_buf();

    apply_patches(
        PatchSource::local_path(workspace.path().to_path_buf()),
        Some(manifest_path.clone()),
        None,
    )
    .unwrap();

    let content = workspace.read_manifest();
    let normalized = normalize_manifest(&content, Some(&workspace));
    assert_snapshot!(
        normalized.as_str(),
        @r###"
[workspace]
members = ["crates/rattler-one", "crates/rattler-two", "crates/other-crate"]

[workspace.dependencies]
other-crate = "3.0.0"
rattler-one = "1.0.0"
rattler-two = "2.0.0"

[workspace.metadata]

[workspace.metadata.cargo-patch-source]
original-versions = { other-crate = "3.0.0", rattler-one = "1.0.0", rattler-two = "2.0.0" }
managed-patches = ["crates-io"]

[patch]

[patch.crates-io]
other-crate = { path = "<workspace>/crates/other-crate" }
rattler-one = { path = "<workspace>/crates/rattler-one" }
rattler-two = { path = "<workspace>/crates/rattler-two" }
"###
    );
}

#[test]
fn test_no_matching_crates() {
    let fixture = TestFixture::new();
    let workspace = rattler_workspace(&fixture);
    let project = rattler_project(&fixture);

    let result = apply_patches(
        PatchSource::local_path(workspace.path().to_path_buf()),
        Some(project.manifest_path().to_path_buf()),
        Some("nonexistent-*"),
    );

    let err = result.unwrap_err();
    let err_repr = format!("{:?}", err);
    assert_snapshot!(
        err_repr.as_str(),
        @r###"NoMatchingCrates { pattern: "nonexistent-*" }"###
    );
}

#[test]
fn test_preserves_existing_patches() {
    let fixture = TestFixture::new();
    let workspace = rattler_workspace(&fixture);
    let project = rattler_project(&fixture);

    project.append_manifest(
        r#"
[patch.crates-io]
some-existing-crate = { path = "/some/other/path" }
"#,
    );

    apply_patches(
        PatchSource::local_path(workspace.path().to_path_buf()),
        Some(project.manifest_path().to_path_buf()),
        Some("rattler-*"),
    )
    .unwrap();

    let content_after_apply = project.read_manifest();
    let normalized_after_apply = normalize_manifest(&content_after_apply, Some(&workspace));
    assert_snapshot!(
        normalized_after_apply.as_str(),
        @r###"
[package]
name = "target-project"
version = "0.1.0"
edition = "2021"

[package.metadata]

[package.metadata.cargo-patch-source]
original-versions = { rattler-one = "1.0.0", rattler-two = "2.0.0" }
managed-patches = ["crates-io"]

[dependencies]
other-crate = "3.0.0"
rattler-one = "1.0.0"
rattler-two = "2.0.0"

[patch.crates-io]
some-existing-crate = { path = "/some/other/path" }
rattler-one = { path = "<workspace>/crates/rattler-one" }
rattler-two = { path = "<workspace>/crates/rattler-two" }
"###
    );

    remove_patches(Some(project.manifest_path().to_path_buf())).unwrap();

    let content_after_remove = project.read_manifest();
    let normalized_after_remove = normalize_manifest(&content_after_remove, Some(&workspace));
    assert_snapshot!(
        normalized_after_remove.as_str(),
        @r###"
[package]
name = "target-project"
version = "0.1.0"
edition = "2021"

[dependencies]
other-crate = "3.0.0"
rattler-one = "1.0.0"
rattler-two = "2.0.0"

[patch.crates-io]
some-existing-crate = { path = "/some/other/path" }
"###
    );
}

#[test]
fn test_reapply_prunes_stale_patches() {
    let fixture = TestFixture::new();
    let workspace = rattler_workspace(&fixture);
    let project = rattler_project(&fixture);

    apply_patches(
        PatchSource::local_path(workspace.path().to_path_buf()),
        Some(project.manifest_path().to_path_buf()),
        None,
    )
    .unwrap();

    apply_patches(
        PatchSource::local_path(workspace.path().to_path_buf()),
        Some(project.manifest_path().to_path_buf()),
        Some("rattler-one"),
    )
    .unwrap();

    let content = project.read_manifest();
    let doc: DocumentMut = content.parse().unwrap();

    let patch_table = doc
        .get("patch")
        .and_then(|p| p.get("crates-io"))
        .and_then(|item| item.as_table())
        .cloned()
        .unwrap();

    let mut patched_crates: Vec<_> = patch_table.iter().map(|(k, _)| k.to_string()).collect();
    patched_crates.sort();
    let patched_crates_repr = format!("{:?}", patched_crates);
    assert_snapshot!(
        patched_crates_repr.as_str(),
        @r###"["rattler-one"]"###
    );

    let metadata = doc
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("cargo-patch-source"))
        .map(|item| item.to_string())
        .unwrap();

    assert_snapshot!(
        metadata.as_str(),
        @r###"
        original-versions = { rattler-one = "1.0.0" }
        managed-patches = ["crates-io"]
        "###
    );
}

#[test]
fn test_apply_skips_existing_patch_entries() {
    let fixture = TestFixture::new();
    let workspace = rattler_workspace(&fixture);
    let project = rattler_project(&fixture);

    project.append_manifest(
        r#"
[patch.crates-io]
rattler-one = { path = "/custom/user/path" }
"#,
    );

    apply_patches(
        PatchSource::local_path(workspace.path().to_path_buf()),
        Some(project.manifest_path().to_path_buf()),
        None,
    )
    .unwrap();

    let updated = project.read_manifest();
    let doc: DocumentMut = updated.parse().unwrap();

    let patch_crates_io = doc
        .get("patch")
        .and_then(|p| p.get("crates-io"))
        .and_then(|item| item.as_table())
        .cloned()
        .unwrap();

    let rattler_one_entry = patch_crates_io.get("rattler-one").unwrap().to_string();
    let rattler_one_entry = rattler_one_entry.trim();
    assert_snapshot!(rattler_one_entry, @r###"{ path = "/custom/user/path" }"###);

    let mut patched_crates: Vec<_> = patch_crates_io.iter().map(|(k, _)| k.to_string()).collect();
    patched_crates.sort();
    let patched_crates_repr = format!("{:?}", patched_crates);
    assert_snapshot!(
        patched_crates_repr.as_str(),
        @r###"["other-crate", "rattler-one", "rattler-two"]"###
    );

    let metadata = doc
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("cargo-patch-source"))
        .map(|item| item.to_string())
        .unwrap();

    assert_snapshot!(
        metadata.as_str(),
        @r###"
        original-versions = { other-crate = "3.0.0", rattler-two = "2.0.0" }
        managed-patches = ["crates-io"]
        "###
    );
}

#[test]
fn test_patch_git_dependencies_without_version() {
    let fixture = TestFixture::new();
    let workspace = rattler_workspace(&fixture);
    let project = fixture
        .project("git-deps-project")
        .dep(
            "rattler-one",
            DependencySpec::git("https://github.com/prefix-dev/rattler").tag("v1.0.0"),
        )
        .dep(
            "rattler-two",
            DependencySpec::git("https://github.com/prefix-dev/rattler").tag("v1.0.0"),
        )
        .dep(
            "other-crate",
            DependencySpec::git("https://github.com/prefix-dev/rattler").tag("v1.0.0"),
        )
        .build();

    apply_patches(
        PatchSource::local_path(workspace.path().to_path_buf()),
        Some(project.manifest_path().to_path_buf()),
        None,
    )
    .unwrap();

    let content = project.read_manifest();
    let normalized = normalize_manifest(&content, Some(&workspace));
    assert_snapshot!(
        normalized.as_str(),
        @r###"
[package]
name = "git-deps-project"
version = "0.1.0"
edition = "2021"

[package.metadata]

[package.metadata.cargo-patch-source]
original-versions = { other-crate = "", rattler-one = "", rattler-two = "" }
managed-patches = ["https://github.com/prefix-dev/rattler"]

[dependencies]
other-crate = { git = "https://github.com/prefix-dev/rattler", tag = "v1.0.0" }
rattler-one = { git = "https://github.com/prefix-dev/rattler", tag = "v1.0.0" }
rattler-two = { git = "https://github.com/prefix-dev/rattler", tag = "v1.0.0" }

[patch]

[patch."https://github.com/prefix-dev/rattler"]
other-crate = { path = "<workspace>/crates/other-crate" }
rattler-one = { path = "<workspace>/crates/rattler-one" }
rattler-two = { path = "<workspace>/crates/rattler-two" }
"###
    );
}
