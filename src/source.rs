use std::path::{Path, PathBuf};

/// Path to a source workspace (where we read crates from)
#[derive(Debug, Clone)]
pub struct SourceWorkspacePath(PathBuf);

impl SourceWorkspacePath {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for SourceWorkspacePath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

/// Path to a target manifest (Cargo.toml we're patching)
#[derive(Debug, Clone)]
pub struct TargetManifestPath(PathBuf);

impl TargetManifestPath {
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for TargetManifestPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

/// Represents the source of patches
#[derive(Debug, Clone)]
pub enum PatchSource {
    /// Local filesystem path to a workspace (where we read crates from)
    LocalPath(SourceWorkspacePath),
    /// Git repository URL with optional reference
    Git {
        url: String,
        reference: Option<GitReference>,
    },
}

/// Git reference types
#[derive(Debug, Clone)]
pub enum GitReference {
    Branch(String),
    Tag(String),
    Rev(String),
}

impl PatchSource {
    /// Create a local path source
    pub fn local_path(path: PathBuf) -> Self {
        Self::LocalPath(SourceWorkspacePath::new(path))
    }

    /// Create a git source
    pub fn git(url: String, reference: Option<GitReference>) -> Self {
        Self::Git { url, reference }
    }

    /// Check if this is a local path source
    pub fn is_local(&self) -> bool {
        matches!(self, Self::LocalPath(_))
    }

    /// Check if this is a git source
    pub fn is_git(&self) -> bool {
        matches!(self, Self::Git { .. })
    }
}
