use std::path::PathBuf;

/// Represents the source of patches
#[derive(Debug, Clone)]
pub enum PatchSource {
    /// Local filesystem path to a workspace
    LocalPath(PathBuf),
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
        Self::LocalPath(path)
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
